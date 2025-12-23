'use client';

import { useRef, useEffect, useCallback } from 'react';
import { ClientMessage } from '@/lib/types';

interface BrowserViewerProps {
  onFrame: (callback: (data: string) => void) => void;
  sendInput: (message: ClientMessage) => void;
  isInteractive: boolean;
  viewportWidth?: number;
  viewportHeight?: number;
}

export function BrowserViewer({
  onFrame,
  sendInput,
  isInteractive,
  viewportWidth = 1280,
  viewportHeight = 720
}: BrowserViewerProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  const getScale = useCallback(() => {
    const canvas = canvasRef.current;
    if (!canvas) return { scaleX: 1, scaleY: 1 };
    const rect = canvas.getBoundingClientRect();
    return {
      scaleX: viewportWidth / rect.width,
      scaleY: viewportHeight / rect.height
    };
  }, [viewportWidth, viewportHeight]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    onFrame((data: string) => {
      const img = new Image();
      img.onload = () => {
        ctx.drawImage(img, 0, 0, canvas.width, canvas.height);
      };
      img.src = `data:image/jpeg;base64,${data}`;
    });
  }, [onFrame]);

  const handleMouseEvent = useCallback((
    e: React.MouseEvent<HTMLCanvasElement>,
    action: string
  ) => {
    if (!isInteractive) return;

    const rect = canvasRef.current?.getBoundingClientRect();
    if (!rect) return;

    const { scaleX, scaleY } = getScale();
    const x = Math.round((e.clientX - rect.left) * scaleX);
    const y = Math.round((e.clientY - rect.top) * scaleY);

    sendInput({
      type: 'mouse',
      action,
      x,
      y,
      button: e.button === 0 ? 'left' : e.button === 2 ? 'right' : 'middle'
    });
  }, [isInteractive, getScale, sendInput]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (!isInteractive) return;
    e.preventDefault();

    const isPrintable = e.key.length === 1;

    sendInput({
      type: 'keyboard',
      action: 'keyDown',
      key: e.key
    });

    if (isPrintable) {
      sendInput({
        type: 'keyboard',
        action: 'char',
        key: e.key,
        text: e.key
      });
    }
  }, [isInteractive, sendInput]);

  const handleKeyUp = useCallback((e: React.KeyboardEvent) => {
    if (!isInteractive) return;
    e.preventDefault();

    sendInput({
      type: 'keyboard',
      action: 'keyUp',
      key: e.key
    });
  }, [isInteractive, sendInput]);

  const handleWheel = useCallback((e: React.WheelEvent<HTMLCanvasElement>) => {
    if (!isInteractive) return;
    e.preventDefault();

    const rect = canvasRef.current?.getBoundingClientRect();
    if (!rect) return;

    const { scaleX, scaleY } = getScale();
    const x = Math.round((e.clientX - rect.left) * scaleX);
    const y = Math.round((e.clientY - rect.top) * scaleY);

    sendInput({
      type: 'scroll',
      x,
      y,
      deltaX: e.deltaX,
      deltaY: e.deltaY
    });
  }, [isInteractive, getScale, sendInput]);

  return (
    <div className={`relative border rounded-lg overflow-hidden bg-gray-900 ${
      isInteractive ? 'ring-2 ring-blue-500' : ''
    }`}>
      <canvas
        ref={canvasRef}
        width={viewportWidth}
        height={viewportHeight}
        tabIndex={0}
        className="w-full h-auto cursor-default focus:outline-none"
        style={{ 
          cursor: isInteractive ? 'crosshair' : 'default',
          aspectRatio: `${viewportWidth} / ${viewportHeight}`
        }}
        onMouseDown={(e) => handleMouseEvent(e, 'mousePressed')}
        onMouseUp={(e) => handleMouseEvent(e, 'mouseReleased')}
        onMouseMove={(e) => handleMouseEvent(e, 'mouseMoved')}
        onWheel={handleWheel}
        onKeyDown={handleKeyDown}
        onKeyUp={handleKeyUp}
        onContextMenu={(e) => e.preventDefault()}
      />
      {isInteractive && (
        <div className="absolute top-2 left-2 bg-blue-500 text-white px-2 py-1 rounded text-sm font-medium">
          Interactive Mode - Click and type to control
        </div>
      )}
    </div>
  );
}
