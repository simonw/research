'use client';

import { useEffect, useState } from 'react';
import { MousePointer2 } from 'lucide-react';

interface RemoteCursorProps {
  x: number;
  y: number;
  color: 'blue' | 'black';
  isClicking: boolean;
  visible: boolean;
}

export function RemoteCursor({ x, y, color, isClicking, visible }: RemoteCursorProps) {
  const [smoothPosition, setSmoothPosition] = useState({ x, y });
  
  useEffect(() => {
    setSmoothPosition({ x, y });
  }, [x, y]);

  if (!visible) return null;

  const fillColor = color === 'blue' ? '#3B82F6' : '#1a1a1a';
  
  return (
    <div
      className="pointer-events-none absolute z-50"
      style={{
        left: smoothPosition.x,
        top: smoothPosition.y,
        transform: `translate(-2px, -2px) scale(${isClicking ? 0.85 : 1})`,
        transition: 'left 0.1s ease-out, top 0.1s ease-out, transform 0.1s ease-out',
      }}
    >
      <div
        style={{
          filter: 'drop-shadow(0 2px 4px rgba(0,0,0,0.2))',
        }}
      >
        <MousePointer2
          size={28}
          fill={fillColor}
          color="white"
          strokeWidth={2}
        />
      </div>
      
      {isClicking && (
        <div
          className="absolute top-0 left-0 w-8 h-8 -translate-x-1 -translate-y-1 rounded-full animate-ping"
          style={{
            backgroundColor: color === 'blue' ? 'rgba(59, 130, 246, 0.3)' : 'rgba(0, 0, 0, 0.2)',
          }}
        />
      )}
    </div>
  );
}
