'use client';

import { useRef, useEffect, useCallback, useState, useMemo } from 'react';
import { ClientMessage } from '@/lib/types';

function debounce<T extends (...args: Parameters<T>) => void>(
  fn: T,
  delay: number
): (...args: Parameters<T>) => void {
  let timeoutId: ReturnType<typeof setTimeout>;
  return (...args: Parameters<T>) => {
    clearTimeout(timeoutId);
    timeoutId = setTimeout(() => fn(...args), delay);
  };
}

interface BrowserViewerProps {
  onFrame: (callback: (data: string) => void) => void;
  sendInput: (message: ClientMessage) => void;
  isInteractive: boolean;
  viewport: { width: number; height: number };
  takeoverMessage?: string;
  onDone?: () => void;
}

const ASPECT_RATIO = 4 / 3;
const MIN_WIDTH = 320;
const MIN_HEIGHT = 240;

export function BrowserViewer({
  onFrame,
  sendInput,
  isInteractive,
  viewport,
  takeoverMessage,
  onDone
}: BrowserViewerProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const measureRef = useRef<HTMLDivElement>(null);
  const [hasRequestedInitialViewport, setHasRequestedInitialViewport] = useState(false);

  const calculateViewportForContainer = useCallback((containerWidth: number, containerHeight: number) => {
    let width = Math.floor(containerWidth);
    let height = Math.round(width / ASPECT_RATIO);

    if (height > containerHeight) {
      height = Math.floor(containerHeight);
      width = Math.round(height * ASPECT_RATIO);
    }

    width = Math.max(MIN_WIDTH, width);
    height = Math.max(MIN_HEIGHT, height);
    height = Math.round(width / ASPECT_RATIO);

    return { width, height };
  }, []);

  const debouncedSendViewport = useMemo(
    () => debounce((width: number, height: number) => {
      sendInput({ type: 'viewport', width, height });
    }, 200),
    [sendInput]
  );

  useEffect(() => {
    const measureEl = measureRef.current;
    if (!measureEl) return;

    const handleResize = (entries: ResizeObserverEntry[]) => {
      const { width: containerWidth, height: containerHeight } = entries[0].contentRect;
      if (containerWidth === 0 || containerHeight === 0) return;

      const { width, height } = calculateViewportForContainer(containerWidth, containerHeight);
      
      if (!hasRequestedInitialViewport) {
        sendInput({ type: 'viewport', width, height });
        setHasRequestedInitialViewport(true);
      } else {
        debouncedSendViewport(width, height);
      }
    };

    const observer = new ResizeObserver(handleResize);
    observer.observe(measureEl);

    return () => observer.disconnect();
  }, [calculateViewportForContainer, debouncedSendViewport, hasRequestedInitialViewport, sendInput]);

  const getScale = useCallback(() => {
    const canvas = canvasRef.current;
    if (!canvas) return { scaleX: 1, scaleY: 1 };
    const rect = canvas.getBoundingClientRect();
    if (rect.width === 0 || rect.height === 0) return { scaleX: 1, scaleY: 1 };
    return {
      scaleX: viewport.width / rect.width,
      scaleY: viewport.height / rect.height
    };
  }, [viewport.width, viewport.height]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    onFrame((data: string) => {
      const img = new Image();
      img.onload = () => {
        const currentCanvas = canvasRef.current;
        if (currentCanvas) {
          const currentCtx = currentCanvas.getContext('2d');
          if (currentCtx) {
            currentCtx.drawImage(img, 0, 0, currentCanvas.width, currentCanvas.height);
          }
        }
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

    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'v') {
      return;
    }

    e.preventDefault();

    const isPrintable = e.key.length === 1;

    sendInput({
      type: 'keyboard',
      action: 'keyDown',
      key: e.key,
      code: e.code,
      keyCode: e.keyCode
    });

    if (isPrintable) {
      sendInput({
        type: 'keyboard',
        action: 'char',
        key: e.key,
        text: e.key,
        code: e.code,
        keyCode: e.keyCode
      });
    }
  }, [isInteractive, sendInput]);

  const handleKeyUp = useCallback((e: React.KeyboardEvent) => {
    if (!isInteractive) return;
    e.preventDefault();

    sendInput({
      type: 'keyboard',
      action: 'keyUp',
      key: e.key,
      code: e.code,
      keyCode: e.keyCode
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

  const handlePaste = useCallback((e: React.ClipboardEvent) => {
    if (!isInteractive) return;
    e.preventDefault();
    const text = e.clipboardData.getData('text');
    if (text) {
      sendInput({
        type: 'paste',
        text
      });
    }
  }, [isInteractive, sendInput]);

  return (
    <div className="w-full flex flex-col">
      {isInteractive && takeoverMessage && (
        <div className="bg-surface border border-accent/30 rounded-lg p-4 shadow-sm animate-in fade-in slide-in-from-top-2 mb-4">
          <h3 className="font-semibold text-accent mb-1 text-sm">Action Required</h3>
          <p className="text-sm text-foreground mb-3">{takeoverMessage}</p>
          {onDone && (
            <button
              onClick={onDone}
              className="w-full py-2 px-4 bg-accent hover:bg-accent-hover text-white rounded-md text-sm font-medium transition-colors"
            >
              I'm Done
            </button>
          )}
        </div>
      )}

      <div 
        ref={measureRef} 
        className="w-full h-[500px] flex items-center justify-center"
      >
        <div
          ref={containerRef}
          className={`relative bg-surface border border-border rounded-lg shadow-lg overflow-hidden ${
            isInteractive ? 'ring-2 ring-accent' : ''
          }`}
          style={{
            width: viewport.width,
            height: viewport.height,
          }}
        >
          <canvas
            ref={canvasRef}
            width={viewport.width}
            height={viewport.height}
            tabIndex={0}
            className="cursor-default focus:outline-none bg-white block"
            onMouseDown={(e) => handleMouseEvent(e, 'mousePressed')}
            onMouseUp={(e) => handleMouseEvent(e, 'mouseReleased')}
            onMouseMove={(e) => handleMouseEvent(e, 'mouseMoved')}
            onWheel={handleWheel}
            onKeyDown={handleKeyDown}
            onKeyUp={handleKeyUp}
            onPaste={handlePaste}
            onContextMenu={(e) => e.preventDefault()}
          />
          {isInteractive && (
            <div className="absolute top-2 left-2 right-2 flex justify-center pointer-events-none">
              <div className="bg-accent/90 text-white px-3 py-1 rounded-full text-xs font-medium shadow-sm backdrop-blur-sm">
                Interactive Mode
              </div>
            </div>
          )}
          <div className="absolute bottom-2 right-2 pointer-events-none">
            <div className="bg-black/50 text-white px-2 py-0.5 rounded text-xs font-mono backdrop-blur-sm">
              {viewport.width}×{viewport.height}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
