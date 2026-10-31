import React from 'react';

interface AuroraBackgroundProps {
  showAurora: boolean;
  showNoise: boolean;
  showGrid: boolean;
  isPrivate?: boolean;
}

/**
 * Nappe d'ambiance Aurora : trois halos lents + grain filmique + grille technique.
 * Purement décoratif, `pointer-events: none`, rendu GPU-friendly.
 */
export const AuroraBackground: React.FC<AuroraBackgroundProps> = ({
  showAurora,
  showNoise,
  showGrid,
  isPrivate,
}) => (
  <>
    {showAurora && (
      <div
        className="aurora-stage"
        aria-hidden
        style={
          isPrivate
            ? {
                // Ambiance violette dédiée aux onglets fantômes
                ['--aurora-1' as string]: 'rgba(139, 92, 246, 0.5)',
                ['--aurora-2' as string]: 'rgba(217, 70, 239, 0.34)',
                ['--aurora-3' as string]: 'rgba(56, 189, 248, 0.22)',
              }
            : undefined
        }
      >
        <span className="aurora-blob b1" />
        <span className="aurora-blob b2" />
        <span className="aurora-blob b3" />
      </div>
    )}
    {showNoise && <div className="noise-overlay" aria-hidden />}
    {showGrid && <div className="grid-overlay" aria-hidden />}
  </>
);
