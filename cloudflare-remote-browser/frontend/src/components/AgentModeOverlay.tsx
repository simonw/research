'use client';

interface AgentModeOverlayProps {
  visible: boolean;
  width: number;
  height: number;
}

export function AgentModeOverlay({ visible, width, height }: AgentModeOverlayProps) {
  if (!visible) return null;

  return (
    <>
      <style jsx>{`
        @keyframes borderRotate {
          0% {
            --angle: 0deg;
          }
          100% {
            --angle: 360deg;
          }
        }
        
        @keyframes pulseGlow {
          0%, 100% {
            opacity: 0.6;
          }
          50% {
            opacity: 1;
          }
        }
        
        .agent-border {
          position: absolute;
          inset: 0;
          border-radius: 0.5rem;
          padding: 3px;
          background: linear-gradient(
            var(--angle, 0deg),
            #3B82F6 0%,
            #60A5FA 25%,
            #93C5FD 50%,
            #60A5FA 75%,
            #3B82F6 100%
          );
          -webkit-mask: 
            linear-gradient(#fff 0 0) content-box, 
            linear-gradient(#fff 0 0);
          mask: 
            linear-gradient(#fff 0 0) content-box, 
            linear-gradient(#fff 0 0);
          -webkit-mask-composite: xor;
          mask-composite: exclude;
          animation: borderRotate 3s linear infinite, pulseGlow 2s ease-in-out infinite;
          pointer-events: none;
        }
        
        @property --angle {
          syntax: '<angle>';
          initial-value: 0deg;
          inherits: false;
        }
      `}</style>
      
      <div className="agent-border" />
      
      <div 
        className="absolute inset-0 rounded-lg pointer-events-none"
        style={{
          boxShadow: 'inset 0 0 30px rgba(59, 130, 246, 0.15), 0 0 20px rgba(59, 130, 246, 0.2)',
        }}
      />
    </>
  );
}
