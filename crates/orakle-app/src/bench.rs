use std::fs;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::{Duration, Instant};

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
use webkit2gtk::WebViewExt as WebKitWebViewExt;
#[cfg(target_os = "linux")]
use wry::{WebViewBuilderExtUnix, WebViewExtUnix};

use crate::metrics::{
    capture_memory_cpu_snapshot, summarize, MemoryCpuSnapshot, SummaryPercentiles,
};

#[derive(Debug, Clone, serde::Serialize)]
pub struct BenchSample {
    pub snapshot: MemoryCpuSnapshot,
    pub action_latency_ms: f64,
    pub step_elapsed_ms: f64,
    pub cpu_delta_ms: u64,
    pub cpu_pct: f64,
}

#[derive(Debug, serde::Serialize)]
pub struct ScenarioAggregate {
    pub scenario: String,
    pub runs: usize,
    pub warmup_runs: usize,
    pub total_rss_kb: SummaryPercentiles,
    pub total_pss_kb: SummaryPercentiles,
    pub total_swap_kb: SummaryPercentiles,
    pub main_rss_kb: SummaryPercentiles,
    pub webkit_web_rss_kb: SummaryPercentiles,
    pub webkit_net_rss_kb: SummaryPercentiles,
    pub webkit_gpu_rss_kb: SummaryPercentiles,
    pub cgroup_kb: SummaryPercentiles,
    pub cgroup_anon_kb: SummaryPercentiles,
    pub cgroup_file_kb: SummaryPercentiles,
    pub cgroup_shmem_kb: SummaryPercentiles,
    pub cgroup_kernel_kb: SummaryPercentiles,
    pub cpu_ms: SummaryPercentiles,
    pub cpu_pct: SummaryPercentiles,
    pub action_latency_ms: SummaryPercentiles,
    pub samples: Vec<BenchSample>,
}

#[derive(Debug, serde::Serialize)]
pub struct BenchSuiteReport {
    pub version: String,
    pub os: String,
    pub arch: String,
    pub warmup_runs: usize,
    pub measured_runs: usize,
    pub startup_window_ms: f64,
    pub cold_first_webview_ms: f64,
    pub scenarios: Vec<ScenarioAggregate>,
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

fn release_views(views: &mut Vec<WebView>, #[cfg(target_os = "linux")] fixed: &gtk::Fixed) {
    for wv in views.drain(..) {
        #[cfg(target_os = "linux")]
        {
            let gwv = wv.webview();
            gwv.terminate_web_process();
            fixed.remove(&gwv);
        }
        drop(wv);
    }
    #[cfg(target_os = "linux")]
    while gtk::events_pending() {
        gtk::main_iteration_do(false);
    }
}

#[derive(Debug, Clone)]
enum BenchTick {
    Step(usize),
}

pub fn run_memory_benchmark_suite(proxy_port: u16, runs: usize, out_path: Option<PathBuf>) {
    let suite_start = Instant::now();
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

    let startup_window_ms = suite_start.elapsed().as_secs_f64() * 1000.0;
    let tmp_dir = tempfile::tempdir().expect("bench tmpdir");
    let mut ctx = WebContext::new(Some(tmp_dir.path().to_path_buf()));
    let mut views: Vec<WebView> = Vec::new();

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
    let warmup_runs: usize = 1;
    let total_runs = warmup_runs + runs;
    let total_steps = total_runs * scenarios.len();
    let step_settle_ms: u64 = if runs >= 15 { 600 } else { 1100 };

    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(400));
        for idx in 0..=total_steps {
            let _ = proxy_ev.send_event(BenchTick::Step(idx));
            std::thread::sleep(Duration::from_millis(step_settle_ms));
        }
    });

    let mut per_scenario: Vec<Vec<BenchSample>> = vec![Vec::new(); scenarios.len()];
    let mut active_step: Option<(usize, usize, f64, Instant, u64)> = None;
    let mut cold_first_webview_ms: f64 = 0.0;

    event_loop.run(move |ev, _, cf| {
        *cf = ControlFlow::Wait;
        match ev {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *cf = ControlFlow::Exit,
            Event::UserEvent(BenchTick::Step(idx)) => {
                if let Some((run_i, sc_i, action_ms, started_at, prev_cpu_ms)) = active_step.take() {
                    let snap = capture_memory_cpu_snapshot();
                    let elapsed_ms = started_at.elapsed().as_secs_f64() * 1000.0;
                    let cpu_delta = snap.total_cpu_ms.saturating_sub(prev_cpu_ms);
                    let cpu_pct = if elapsed_ms > 1.0 {
                        (cpu_delta as f64 / elapsed_ms) * 100.0
                    } else {
                        0.0
                    };
                    if run_i == 0 && sc_i == 0 {
                        cold_first_webview_ms = elapsed_ms;
                    }
                    // On exclut la passe de warmup (run_i < warmup_runs) des statistiques
                    if run_i >= warmup_runs {
                        per_scenario[sc_i].push(BenchSample {
                            snapshot: snap,
                            action_latency_ms: action_ms,
                            step_elapsed_ms: elapsed_ms,
                            cpu_delta_ms: cpu_delta,
                            cpu_pct,
                        });
                    }
                }

                if idx >= total_steps {
                    let mut aggregates = Vec::new();
                    println!(
                        "\n=== [ORAKLE V2 BENCHMARK RÉEL MÉMOIRE / PSS / CGROUP / LATENCE ({} runs mesurés + {} warmup)] ===",
                        runs, warmup_runs
                    );
                    println!("cold_window_ms={:.1} | cold_first_webview_ms={:.1}", startup_window_ms, cold_first_webview_ms);
                    println!("| Scénario | RSS p50 (p90/p95) MiB | PSS p50 (p90/p95) MiB | cgroup p50 MiB | Swap p50 MiB | Latence action p50/p95 ms | CPU % p50 |");
                    println!("|---|---|---|---|---|---|---|");

                    for (si, name) in scenarios.iter().enumerate() {
                        let s = &per_scenario[si];
                        let rss_kb: Vec<f64> = s.iter().map(|x| x.snapshot.total_rss_kb as f64).collect();
                        let pss_kb: Vec<f64> = s.iter().map(|x| x.snapshot.total_pss_kb as f64).collect();
                        let swap_kb: Vec<f64> = s.iter().map(|x| x.snapshot.total_swap_kb as f64).collect();
                        let main_kb: Vec<f64> = s.iter().map(|x| x.snapshot.main.rss_kb as f64).collect();
                        let web_kb: Vec<f64> = s.iter().map(|x| x.snapshot.webkit_web.rss_kb as f64).collect();
                        let net_kb: Vec<f64> = s.iter().map(|x| x.snapshot.webkit_network.rss_kb as f64).collect();
                        let gpu_kb: Vec<f64> = s.iter().map(|x| x.snapshot.webkit_gpu.rss_kb as f64).collect();
                        let cg_kb: Vec<f64> = s
                            .iter()
                            .filter_map(|x| x.snapshot.cgroup_memory_current_kb.map(|k| k as f64))
                            .collect();
                        let cg_anon: Vec<f64> = s
                            .iter()
                            .filter_map(|x| x.snapshot.cgroup_breakdown.as_ref().map(|b| b.anon_kb as f64))
                            .collect();
                        let cg_file: Vec<f64> = s
                            .iter()
                            .filter_map(|x| x.snapshot.cgroup_breakdown.as_ref().map(|b| b.file_kb as f64))
                            .collect();
                        let cg_shmem: Vec<f64> = s
                            .iter()
                            .filter_map(|x| x.snapshot.cgroup_breakdown.as_ref().map(|b| b.shmem_kb as f64))
                            .collect();
                        let cg_kernel: Vec<f64> = s
                            .iter()
                            .filter_map(|x| x.snapshot.cgroup_breakdown.as_ref().map(|b| b.kernel_kb as f64))
                            .collect();
                        let cpus: Vec<f64> = s.iter().map(|x| x.snapshot.total_cpu_ms as f64).collect();
                        let cpu_pcts: Vec<f64> = s.iter().map(|x| x.cpu_pct).collect();
                        let lats: Vec<f64> = s.iter().map(|x| x.action_latency_ms).collect();

                        let agg = ScenarioAggregate {
                            scenario: (*name).to_string(),
                            runs: s.len(),
                            warmup_runs,
                            total_rss_kb: summarize(&rss_kb),
                            total_pss_kb: summarize(&pss_kb),
                            total_swap_kb: summarize(&swap_kb),
                            main_rss_kb: summarize(&main_kb),
                            webkit_web_rss_kb: summarize(&web_kb),
                            webkit_net_rss_kb: summarize(&net_kb),
                            webkit_gpu_rss_kb: summarize(&gpu_kb),
                            cgroup_kb: summarize(&cg_kb),
                            cgroup_anon_kb: summarize(&cg_anon),
                            cgroup_file_kb: summarize(&cg_file),
                            cgroup_shmem_kb: summarize(&cg_shmem),
                            cgroup_kernel_kb: summarize(&cg_kernel),
                            cpu_ms: summarize(&cpus),
                            cpu_pct: summarize(&cpu_pcts),
                            action_latency_ms: summarize(&lats),
                            samples: s.clone(),
                        };

                        println!(
                            "| {} | {:.1} ({:.1}/{:.1}) | {:.1} ({:.1}/{:.1}) | {:.1} | {:.1} | {:.1}/{:.1} | {:.1}% |",
                            name,
                            agg.total_rss_kb.median / 1024.0,
                            agg.total_rss_kb.p90 / 1024.0,
                            agg.total_rss_kb.p95 / 1024.0,
                            agg.total_pss_kb.median / 1024.0,
                            agg.total_pss_kb.p90 / 1024.0,
                            agg.total_pss_kb.p95 / 1024.0,
                            agg.cgroup_kb.median / 1024.0,
                            agg.total_swap_kb.median / 1024.0,
                            agg.action_latency_ms.median,
                            agg.action_latency_ms.p95,
                            agg.cpu_pct.median,
                        );
                        aggregates.push(agg);
                    }

                    let report = BenchSuiteReport {
                        version: "0.2.0-phase5".into(),
                        os: std::env::consts::OS.into(),
                        arch: std::env::consts::ARCH.into(),
                        warmup_runs,
                        measured_runs: runs,
                        startup_window_ms,
                        cold_first_webview_ms,
                        scenarios: aggregates,
                    };
                    if let Ok(json_pretty) = serde_json::to_string_pretty(&report) {
                        if let Some(p) = &out_path {
                            if let Some(parent) = p.parent() {
                                let _ = fs::create_dir_all(parent);
                            }
                            let _ = fs::write(p, &json_pretty);
                            println!("BENCH_FILE_WRITTEN={}", p.display());
                        }
                    }
                    *cf = ControlFlow::Exit;
                    return;
                }

                let run_i = idx / scenarios.len();
                let sc_i = idx % scenarios.len();
                let cpu_before = capture_memory_cpu_snapshot().total_cpu_ms;
                let step_started = Instant::now();

                match sc_i {
                    0 => {
                        release_views(&mut views, #[cfg(target_os = "linux")] &fixed);
                        views.push(build_wv("about:blank", rect(0, 0, 1280, 800), proxy_port, &mut ctx, #[cfg(target_os = "linux")] &fixed, #[cfg(not(target_os = "linux"))] &window));
                    }
                    1 => {
                        release_views(&mut views, #[cfg(target_os = "linux")] &fixed);
                        for _ in 0..3 {
                            views.push(build_wv("about:blank", rect(0, 0, 1280, 800), proxy_port, &mut ctx, #[cfg(target_os = "linux")] &fixed, #[cfg(not(target_os = "linux"))] &window));
                        }
                    }
                    2 => {
                        release_views(&mut views, #[cfg(target_os = "linux")] &fixed);
                        for i in 0..3 {
                            let u = format!("{}?n={}", simple_url, i);
                            views.push(build_wv(&u, rect(0, 0, 1280, 800), proxy_port, &mut ctx, #[cfg(target_os = "linux")] &fixed, #[cfg(not(target_os = "linux"))] &window));
                        }
                    }
                    3 => {
                        release_views(&mut views, #[cfg(target_os = "linux")] &fixed);
                        for i in 0..3 {
                            let u = format!("{}?n={}", complex_url, i);
                            views.push(build_wv(&u, rect(0, 0, 1280, 800), proxy_port, &mut ctx, #[cfg(target_os = "linux")] &fixed, #[cfg(not(target_os = "linux"))] &window));
                        }
                    }
                    4 => {
                        release_views(&mut views, #[cfg(target_os = "linux")] &fixed);
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
                        release_views(&mut views, #[cfg(target_os = "linux")] &fixed);
                        views.push(build_wv(&format!("{}?pane=left", complex_url), rect(0, 0, 640, 800), proxy_port, &mut ctx, #[cfg(target_os = "linux")] &fixed, #[cfg(not(target_os = "linux"))] &window));
                        views.push(build_wv(&format!("{}?pane=right", complex_url), rect(640, 0, 640, 800), proxy_port, &mut ctx, #[cfg(target_os = "linux")] &fixed, #[cfg(not(target_os = "linux"))] &window));
                    }
                    6 => {
                        if let Some(wv) = views.pop() {
                            #[cfg(target_os = "linux")]
                            {
                                let gwv = wv.webview();
                                gwv.terminate_web_process();
                                fixed.remove(&gwv);
                            }
                            drop(wv);
                            #[cfg(target_os = "linux")]
                            while gtk::events_pending() {
                                gtk::main_iteration_do(false);
                            }
                            let _ = views[0].set_bounds(rect(0, 0, 1280, 800));
                        }
                    }
                    7 => {
                        if let Some(left) = views.first() {
                            let _ = left.set_bounds(rect(0, 0, 640, 800));
                        }
                        views.push(build_wv(&format!("{}?pane=restored", complex_url), rect(640, 0, 640, 800), proxy_port, &mut ctx, #[cfg(target_os = "linux")] &fixed, #[cfg(not(target_os = "linux"))] &window));
                    }
                    _ => {}
                }

                let action_ms = step_started.elapsed().as_secs_f64() * 1000.0;
                active_step = Some((run_i, sc_i, action_ms, step_started, cpu_before));
            }
            _ => {}
        }
    });
}
