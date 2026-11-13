use std::net::SocketAddr;
use std::time::Duration;

use tao::dpi::{LogicalPosition, LogicalSize};
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::window::WindowBuilder;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use wry::{
    ProxyConfig as WryProxyConfig, ProxyEndpoint, Rect, WebContext, WebView, WebViewBuilder,
};

#[cfg(target_os = "linux")]
use gtk::prelude::*;
#[cfg(target_os = "linux")]
use tao::platform::unix::WindowExtUnix;
#[cfg(target_os = "linux")]
use wry::WebViewBuilderExtUnix;

use crate::metrics::{
    capture_memory_cpu_snapshot, summarize, MemoryCpuSnapshot, SummaryPercentiles,
};

#[derive(Debug, serde::Serialize)]
pub struct ScenarioAggregate {
    pub scenario: String,
    pub runs: usize,
    pub total_rss_kb: SummaryPercentiles,
    pub total_pss_kb: SummaryPercentiles,
    pub main_rss_kb: SummaryPercentiles,
    pub webkit_web_rss_kb: SummaryPercentiles,
    pub webkit_net_rss_kb: SummaryPercentiles,
    pub webkit_gpu_rss_kb: SummaryPercentiles,
    pub cgroup_kb: SummaryPercentiles,
    pub cpu_ms: SummaryPercentiles,
    pub samples: Vec<MemoryCpuSnapshot>,
}

async fn spawn_bench_server() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind bench server");
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        let mut complex_html = String::from(
            "<!doctype html><html><head><title>Complex Bench</title><style>.box{display:inline-block;width:24px;height:24px;margin:2px;background:#0969da;border-radius:4px;}</style></head><body><h1>Complex DOM + Canvas</h1><canvas id='c' width='640' height='240'></canvas><div id='grid'>",
        );
        for i in 0..1500 {
            complex_html.push_str(&format!("<span class='box' data-i='{}'></span>", i));
        }
        complex_html.push_str(
            "</div><script>const ctx=document.getElementById('c').getContext('2d');for(let i=0;i<200;i++){ctx.fillStyle='#10b981';ctx.fillRect(i*3,10,2,200);}window.__buf=new Array(350000).fill('orakle-bench-node');</script></body></html>",
        );

        loop {
            if let Ok((mut s, _)) = listener.accept().await {
                let complex_clone = complex_html.clone();
                tokio::spawn(async move {
                    let mut buf = [0u8; 2048];
                    let n = s.read(&mut buf).await.unwrap_or(0);
                    if n == 0 {
                        return;
                    }
                    let req = String::from_utf8_lossy(&buf[..n]);
                    let path = req
                        .lines()
                        .next()
                        .unwrap_or("")
                        .split_whitespace()
                        .nth(1)
                        .unwrap_or("/");
                    let body = if path.starts_with("/complex") {
                        complex_clone
                    } else {
                        "<!doctype html><html><head><title>Simple Page</title></head><body><h1>Simple Orakle Page</h1><p>Minimal baseline HTML document.</p></body></html>".to_string()
                    };
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    let _ = s.write_all(resp.as_bytes()).await;
                });
            }
        }
    });

    addr
}

fn rect(x: i32, y: i32, w: i32, h: i32) -> Rect {
    Rect {
        position: LogicalPosition::new(x, y).into(),
        size: LogicalSize::new(w.max(100), h.max(100)).into(),
    }
}

fn build_wv(
    url: &str,
    bounds: Rect,
    proxy_port: u16,
    ctx: &mut WebContext,
    #[cfg(target_os = "linux")] fixed: &gtk::Fixed,
    #[cfg(not(target_os = "linux"))] window: &tao::window::Window,
) -> WebView {
    let b = WebViewBuilder::new_with_web_context(ctx)
        .with_url(url)
        .with_bounds(bounds)
        .with_proxy_config(WryProxyConfig::Http(ProxyEndpoint {
            host: "127.0.0.1".into(),
            port: proxy_port.to_string(),
        }));
    #[cfg(target_os = "linux")]
    {
        b.build_gtk(fixed).expect("bench build_gtk")
    }
    #[cfg(not(target_os = "linux"))]
    {
        b.build_as_child(window).expect("bench build_as_child")
    }
}

#[derive(Debug, Clone)]
enum BenchTick {
    Step(usize),
}

pub fn run_memory_benchmark_suite(proxy_port: u16, runs: usize) {
    let rt = tokio::runtime::Runtime::new().expect("bench tokio");
    let bench_addr = rt.block_on(spawn_bench_server());
    let simple_url = format!("http://{}/simple", bench_addr);
    let complex_url = format!("http://{}/complex", bench_addr);

    let event_loop = EventLoopBuilder::<BenchTick>::with_user_event().build();
    let proxy_ev = event_loop.create_proxy();

    let window = WindowBuilder::new()
        .with_title("Orakle Memory & CPU Benchmark Harness")
        .with_inner_size(LogicalSize::new(1280.0, 800.0))
        .build(&event_loop)
        .expect("bench window");

    #[cfg(target_os = "linux")]
    let fixed = {
        let vbox = window.default_vbox().expect("vbox");
        let f = gtk::Fixed::new();
        vbox.pack_start(&f, true, true, 0);
        f.show_all();
        f
    };

    let tmp_dir = tempfile::tempdir().expect("bench tmpdir");
    let mut ctx = WebContext::new(Some(tmp_dir.path().to_path_buf()));
    let mut views: Vec<WebView> = Vec::new();

    // 8 scénarios * N runs
    // Par itération:
    //  0 -> 1 WebView vide (about:blank)
    //  1 -> 3 WebViews actives (about:blank)
    //  2 -> 3 pages simples
    //  3 -> 3 pages complexes
    //  4 -> 10 onglets ouverts (1 visible + 9 en arrière-plan)
    //  5 -> Split View (2 WebViews visibles 50/50)
    //  6 -> Suspension (destruction de 1 WebView du Split -> 1 WebView)
    //  7 -> Restauration (recréation du volet droit -> 2 WebViews)
    let scenarios = [
        "1_webview_vide",
        "3_webviews_vides",
        "3_pages_simples",
        "3_pages_complexes",
        "10_onglets_ouverts",
        "split_view_2_panneaux",
        "apres_suspension_split",
        "apres_restauration_split",
    ];
    let total_steps = runs * 8;

    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(400));
        for idx in 0..=total_steps {
            let _ = proxy_ev.send_event(BenchTick::Step(idx));
            std::thread::sleep(Duration::from_millis(1200));
        }
    });

    let mut per_scenario: Vec<Vec<MemoryCpuSnapshot>> = vec![Vec::new(); 8];
    let mut active_step: Option<(usize, usize)> = None;

    event_loop.run(move |ev, _, cf| {
        *cf = ControlFlow::Wait;
        match ev {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *cf = ControlFlow::Exit,
            Event::UserEvent(BenchTick::Step(idx)) => {
                // Capture snapshot from PREVIOUS step after 1.2s settle!
                if let Some((_run_i, sc_i)) = active_step.take() {
                    let snap = capture_memory_cpu_snapshot();
                    per_scenario[sc_i].push(snap);
                }

                if idx >= total_steps {
                    let mut aggregates = Vec::new();
                    println!("\n=== [ORAKLE V2 BENCHMARK RÉEL MÉMOIRE / PSS / CPU / CGROUP ({} runs)] ===", runs);
                    println!("| Scénario | Total RSS médian (MiB) | Total PSS médian (MiB) | Main RSS (MiB) | WebKitWeb RSS (MiB) | WebKitNet RSS (MiB) | WebKitGPU RSS (MiB) | cgroup memory.current (MiB) | CPU médian (ms) |");
                    println!("|---|---|---|---|---|---|---|---|---|");

                    for (si, name) in scenarios.iter().enumerate() {
                        let s = &per_scenario[si];
                        let rss_mibs: Vec<f64> = s.iter().map(|x| x.total_rss_kb as f64 / 1024.0).collect();
                        let pss_mibs: Vec<f64> = s.iter().map(|x| x.total_pss_kb as f64 / 1024.0).collect();
                        let main_mibs: Vec<f64> = s.iter().map(|x| x.main.rss_kb as f64 / 1024.0).collect();
                        let web_mibs: Vec<f64> = s.iter().map(|x| x.webkit_web.rss_kb as f64 / 1024.0).collect();
                        let net_mibs: Vec<f64> = s.iter().map(|x| x.webkit_network.rss_kb as f64 / 1024.0).collect();
                        let gpu_mibs: Vec<f64> = s.iter().map(|x| x.webkit_gpu.rss_kb as f64 / 1024.0).collect();
                        let cg_mibs: Vec<f64> = s.iter().filter_map(|x| x.cgroup_memory_current_kb.map(|k| k as f64 / 1024.0)).collect();
                        let cpus: Vec<f64> = s.iter().map(|x| x.total_cpu_ms as f64).collect();

                        let agg = ScenarioAggregate {
                            scenario: (*name).to_string(),
                            runs: s.len(),
                            total_rss_kb: summarize(&s.iter().map(|x| x.total_rss_kb as f64).collect::<Vec<_>>()),
                            total_pss_kb: summarize(&s.iter().map(|x| x.total_pss_kb as f64).collect::<Vec<_>>()),
                            main_rss_kb: summarize(&s.iter().map(|x| x.main.rss_kb as f64).collect::<Vec<_>>()),
                            webkit_web_rss_kb: summarize(&s.iter().map(|x| x.webkit_web.rss_kb as f64).collect::<Vec<_>>()),
                            webkit_net_rss_kb: summarize(&s.iter().map(|x| x.webkit_network.rss_kb as f64).collect::<Vec<_>>()),
                            webkit_gpu_rss_kb: summarize(&s.iter().map(|x| x.webkit_gpu.rss_kb as f64).collect::<Vec<_>>()),
                            cgroup_kb: summarize(&s.iter().filter_map(|x| x.cgroup_memory_current_kb.map(|k| k as f64)).collect::<Vec<_>>()),
                            cpu_ms: summarize(&cpus),
                            samples: s.clone(),
                        };

                        println!(
                            "| {} | {:.1} (p95 {:.1}) | {:.1} (p95 {:.1}) | {:.1} | {:.1} | {:.1} | {:.1} | {:.1} | {:.0} |",
                            name,
                            summarize(&rss_mibs).median,
                            summarize(&rss_mibs).p95,
                            summarize(&pss_mibs).median,
                            summarize(&pss_mibs).p95,
                            summarize(&main_mibs).median,
                            summarize(&web_mibs).median,
                            summarize(&net_mibs).median,
                            summarize(&gpu_mibs).median,
                            summarize(&cg_mibs).median,
                            agg.cpu_ms.median,
                        );
                        aggregates.push(agg);
                    }
                    println!("BENCH_JSON={}", serde_json::to_string(&aggregates).unwrap_or_default());
                    *cf = ControlFlow::Exit;
                    return;
                }

                let run_i = idx / 8;
                let sc_i = idx % 8;
                active_step = Some((run_i, sc_i));

                match sc_i {
                    0 => {
                        views.clear();
                        views.push(build_wv("about:blank", rect(0, 0, 1280, 800), proxy_port, &mut ctx, #[cfg(target_os = "linux")] &fixed, #[cfg(not(target_os = "linux"))] &window));
                    }
                    1 => {
                        views.clear();
                        for _ in 0..3 {
                            views.push(build_wv("about:blank", rect(0, 0, 1280, 800), proxy_port, &mut ctx, #[cfg(target_os = "linux")] &fixed, #[cfg(not(target_os = "linux"))] &window));
                        }
                    }
                    2 => {
                        views.clear();
                        for i in 0..3 {
                            let u = format!("{}?n={}", simple_url, i);
                            views.push(build_wv(&u, rect(0, 0, 1280, 800), proxy_port, &mut ctx, #[cfg(target_os = "linux")] &fixed, #[cfg(not(target_os = "linux"))] &window));
                        }
                    }
                    3 => {
                        views.clear();
                        for i in 0..3 {
                            let u = format!("{}?n={}", complex_url, i);
                            views.push(build_wv(&u, rect(0, 0, 1280, 800), proxy_port, &mut ctx, #[cfg(target_os = "linux")] &fixed, #[cfg(not(target_os = "linux"))] &window));
                        }
                    }
                    4 => {
                        views.clear();
                        for i in 0..10 {
                            let u = format!("{}?tab={}", simple_url, i);
                            let wv = build_wv(&u, rect(0, 0, 1280, 800), proxy_port, &mut ctx, #[cfg(target_os = "linux")] &fixed, #[cfg(not(target_os = "linux"))] &window);
                            if i > 0 {
                                let _ = wv.set_visible(false);
                            }
                            views.push(wv);
                        }
                    }
                    5 => {
                        views.clear();
                        views.push(build_wv(&format!("{}?pane=left", complex_url), rect(0, 0, 640, 800), proxy_port, &mut ctx, #[cfg(target_os = "linux")] &fixed, #[cfg(not(target_os = "linux"))] &window));
                        views.push(build_wv(&format!("{}?pane=right", complex_url), rect(640, 0, 640, 800), proxy_port, &mut ctx, #[cfg(target_os = "linux")] &fixed, #[cfg(not(target_os = "linux"))] &window));
                    }
                    6 => {
                        // Suspension : destruction de la 2e WebView du Split
                        if views.len() > 1 {
                            views.pop();
                            let _ = views[0].set_bounds(rect(0, 0, 1280, 800));
                        }
                    }
                    7 => {
                        // Restauration du volet droit
                        if let Some(left) = views.first() {
                            let _ = left.set_bounds(rect(0, 0, 640, 800));
                        }
                        views.push(build_wv(&format!("{}?pane=restored", complex_url), rect(640, 0, 640, 800), proxy_port, &mut ctx, #[cfg(target_os = "linux")] &fixed, #[cfg(not(target_os = "linux"))] &window));
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    });
}
