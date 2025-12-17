'use client';

import { VMProgress } from '@/lib/types';

interface PhoneFrameProps {
    status: 'idle' | 'loading' | 'ready' | 'stopping';
    progress: VMProgress | null;
    streamUrl: string | null;
    vmName: string;
}

export function PhoneFrame({ status, progress, streamUrl, vmName }: PhoneFrameProps) {
    const progressPercent = progress?.percent ?? 0;

    return (
        <div className="flex flex-col items-center">
            {/* Phone Frame Container - Fixed sizing for 256:480 device aspect ratio */}
            <div
                className="relative bg-gray-900 rounded-[2rem] p-1.5 shadow-2xl shadow-black/50"
                style={{
                    width: '280px',
                    height: '528px', // 280 * (480/256) = 525px, rounded up for padding
                }}
            >
                {/* Inner bezel */}
                <div className="absolute inset-1.5 bg-gray-800 rounded-[1.75rem]" />

                {/* Screen area */}
                <div
                    className="relative w-full h-full bg-black rounded-[1.5rem] overflow-hidden"
                >
                    {status === 'idle' ? (
                        // Idle state - no active sessions
                        <div className="absolute inset-0 flex flex-col items-center justify-center p-6 bg-gradient-to-b from-gray-900 via-gray-900 to-black">
                            {/* Android logo (dimmed) */}
                            <div className="mb-6">
                                <svg className="w-16 h-16 text-gray-600" fill="currentColor" viewBox="0 0 24 24">
                                    <path d="M17.6 9.48l1.84-3.18c.16-.31.04-.69-.26-.85a.637.637 0 0 0-.83.22l-1.88 3.24a11.463 11.463 0 0 0-8.94 0L5.65 5.67a.643.643 0 0 0-.87-.2c-.28.18-.37.54-.22.83L6.4 9.48A10.78 10.78 0 0 0 1 18h22a10.78 10.78 0 0 0-5.4-8.52zM7 15.25a1.25 1.25 0 1 1 0-2.5 1.25 1.25 0 0 1 0 2.5zm10 0a1.25 1.25 0 1 1 0-2.5 1.25 1.25 0 0 1 0 2.5z" />
                                </svg>
                            </div>

                            {/* Message */}
                            <p className="text-gray-500 text-sm font-medium text-center mb-2">
                                No active sessions
                            </p>
                            <p className="text-gray-600 text-xs text-center">
                                Start a new session to begin
                            </p>
                        </div>
                    ) : status === 'loading' ? (
                        // Status display during startup
                        <div className="absolute inset-0 flex flex-col items-center justify-center p-6 bg-gradient-to-b from-gray-900 via-gray-900 to-black">
                            {/* Android logo */}
                            <div className="mb-6">
                                <svg className="w-16 h-16 text-green-500" fill="currentColor" viewBox="0 0 24 24">
                                    <path d="M17.6 9.48l1.84-3.18c.16-.31.04-.69-.26-.85a.637.637 0 0 0-.83.22l-1.88 3.24a11.463 11.463 0 0 0-8.94 0L5.65 5.67a.643.643 0 0 0-.87-.2c-.28.18-.37.54-.22.83L6.4 9.48A10.78 10.78 0 0 0 1 18h22a10.78 10.78 0 0 0-5.4-8.52zM7 15.25a1.25 1.25 0 1 1 0-2.5 1.25 1.25 0 0 1 0 2.5zm10 0a1.25 1.25 0 1 1 0-2.5 1.25 1.25 0 0 1 0 2.5z" />
                                </svg>
                            </div>

                            {/* Status message */}
                            <p className="text-white text-sm font-medium text-center mb-2 min-h-[20px]">
                                {progress?.message || 'Starting...'}
                            </p>

                            {/* Step indicator */}
                            <p className="text-gray-500 text-xs mb-4">
                                {progress ? `Step ${progress.step} of ${progress.totalSteps}` : 'Initializing...'}
                            </p>

                            {/* Progress bar */}
                            <div className="w-full max-w-[200px] h-1 bg-gray-800 rounded-full overflow-hidden">
                                <div
                                    className="h-full bg-gradient-to-r from-green-500 to-emerald-500 transition-all duration-500 ease-out"
                                    style={{ width: `${progressPercent}%` }}
                                />
                            </div>

                            {/* Percentage */}
                            <p className="text-gray-400 text-xs mt-2">
                                {progressPercent}%
                            </p>

                            {/* Stage indicator dots */}
                            <div className="flex gap-1.5 mt-6">
                                {Array.from({ length: 11 }).map((_, i) => (
                                    <div
                                        key={i}
                                        className={`w-1.5 h-1.5 rounded-full transition-all duration-300 ${progress && i < progress.step
                                            ? 'bg-green-500'
                                            : progress && i === progress.step
                                                ? 'bg-green-400 animate-pulse'
                                                : 'bg-gray-700'
                                            }`}
                                    />
                                ))}
                            </div>
                        </div>
                    ) : status === 'stopping' ? (
                        // Stopping state
                        <div className="absolute inset-0 flex flex-col items-center justify-center p-6 bg-gradient-to-b from-gray-900 via-gray-900 to-black">
                            {/* Android logo (red) */}
                            <div className="mb-6">
                                <svg className="w-16 h-16 text-red-500" fill="currentColor" viewBox="0 0 24 24">
                                    <path d="M17.6 9.48l1.84-3.18c.16-.31.04-.69-.26-.85a.637.637 0 0 0-.83.22l-1.88 3.24a11.463 11.463 0 0 0-8.94 0L5.65 5.67a.643.643 0 0 0-.87-.2c-.28.18-.37.54-.22.83L6.4 9.48A10.78 10.78 0 0 0 1 18h22a10.78 10.78 0 0 0-5.4-8.52zM7 15.25a1.25 1.25 0 1 1 0-2.5 1.25 1.25 0 0 1 0 2.5zm10 0a1.25 1.25 0 1 1 0-2.5 1.25 1.25 0 0 1 0 2.5z" />
                                </svg>
                            </div>

                            {/* Message */}
                            <p className="text-red-400 text-sm font-medium text-center mb-2">
                                Stopping...
                            </p>
                            <p className="text-gray-600 text-xs text-center">
                                Session is shutting down
                            </p>
                        </div>
                    ) : (
                        // Embedded ws-scrcpy iframe when ready
                        <iframe
                            src={streamUrl || ''}
                            className="w-full h-full border-0"
                            allow="autoplay; fullscreen"
                            title={`Android Emulator - ${vmName}`}
                            tabIndex={0}
                            onLoad={(e) => {
                                // Auto-focus iframe to enable keyboard input
                                (e.target as HTMLIFrameElement).focus();
                            }}
                        />
                    )}
                </div>

            </div>

            {/* VM name label */}
            <p className="mt-4 text-sm font-mono text-gray-500">{vmName}</p>
        </div>
    );
}
