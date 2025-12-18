'use client';

import { VMProgress } from '@/lib/types';

interface BrowserFrameProps {
    status: 'idle' | 'loading' | 'ready' | 'stopping';
    progress: VMProgress | null;
    streamUrl: string | null;
    vmName: string;
}

export function BrowserFrame({ status, progress, streamUrl, vmName }: BrowserFrameProps) {
    const progressPercent = progress?.percent ?? 0;

    return (
        <div className="flex flex-col items-center w-full">
            <div
                className="relative bg-gray-900 rounded-xl p-1 shadow-2xl shadow-black/50 w-full max-w-[1280px]"
                style={{
                    aspectRatio: '16/9',
                }}
            >
                <div className="h-8 bg-gray-800 rounded-t-lg flex items-center px-4 gap-2">
                    <div className="flex gap-2">
                        <div className="w-3 h-3 rounded-full bg-red-500/80" />
                        <div className="w-3 h-3 rounded-full bg-yellow-500/80" />
                        <div className="w-3 h-3 rounded-full bg-green-500/80" />
                    </div>
                    <div className="ml-4 flex-1 h-5 bg-gray-900/50 rounded flex items-center px-3 text-xs text-gray-500 font-mono">
                        {status === 'ready' ? 'https://remote-browser-session' : 'Connecting...'}
                    </div>
                </div>

                <div
                    className="relative w-full h-[calc(100%-2rem)] bg-black rounded-b-lg overflow-hidden"
                >
                    {status === 'idle' ? (
                        <div className="absolute inset-0 flex flex-col items-center justify-center p-6 bg-gradient-to-b from-gray-900 via-gray-900 to-black">
                            <div className="mb-6">
                                <svg className="w-20 h-20 text-blue-500/80" fill="currentColor" viewBox="0 0 24 24">
                                    <path d="M12 0C5.373 0 0 5.373 0 12s5.373 12 12 12 12-5.373 12-12S18.627 0 12 0zm0 4.364c1.76 0 3.385.602 4.708 1.614l-2.022 3.504c-.676-.482-1.503-.754-2.686-.754-2.227 0-4.036 1.809-4.036 4.036 0 .524.103 1.025.282 1.488L4.85 6.645C6.442 5.232 8.587 4.364 12 4.364zm0 15.272c-2.484 0-4.697-1.182-6.103-3.024l2.584-4.478c.605 1.09 1.76 1.866 3.519 1.866 1.543 0 2.87-.98 3.39-2.35l3.418 1.972c-1.464 2.45-4.137 4.014-7.208 4.014zM16.364 12c0-1.077-.393-2.064-1.037-2.818l3.424-5.932C20.662 4.968 21.818 8.318 21.818 12c0 1.825-.505 3.535-1.393 5.018l-3.327-5.764c.175-.403.266-.84.266-1.254z"/>
                                </svg>
                            </div>

                            <p className="text-gray-500 text-lg font-medium text-center mb-2">
                                No active session
                            </p>
                            <p className="text-gray-600 text-sm text-center">
                                Start a new browser session to begin
                            </p>
                        </div>
                    ) : status === 'loading' ? (
                        <div className="absolute inset-0 flex flex-col items-center justify-center p-6 bg-gradient-to-b from-gray-900 via-gray-900 to-black">
                            <div className="mb-8">
                                <svg className="w-16 h-16 text-blue-500 animate-pulse" fill="currentColor" viewBox="0 0 24 24">
                                    <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm0 18c-4.41 0-8-3.59-8-8s3.59-8 8-8 8 3.59 8 8-3.59 8-8 8zm-5.5-2.5l7.51-3.21 3.21-7.51-7.51 3.21-3.21 7.51zm6.2-4.5c.41 0 .75-.34.75-.75s-.34-.75-.75-.75-.75.34-.75.75.34.75.75.75z"/>
                                </svg>
                            </div>

                            <p className="text-white text-lg font-medium text-center mb-2 min-h-[28px]">
                                {progress?.message || 'Initializing...'}
                            </p>

                            <p className="text-gray-500 text-sm mb-6">
                                {progress ? `Step ${progress.step} of ${progress.totalSteps}` : 'Please wait...'}
                            </p>

                            <div className="w-full max-w-[320px] h-1.5 bg-gray-800 rounded-full overflow-hidden mb-2">
                                <div
                                    className="h-full bg-gradient-to-r from-blue-500 to-indigo-500 transition-all duration-500 ease-out"
                                    style={{ width: `${progressPercent}%` }}
                                />
                            </div>

                            <p className="text-gray-400 text-xs">
                                {progressPercent}% Complete
                            </p>

                            <div className="flex gap-2 mt-8">
                                {Array.from({ length: 8 }).map((_, i) => (
                                    <div
                                        key={i}
                                        className={`w-2 h-2 rounded-full transition-all duration-300 ${progress && i < (progress.step / 2)
                                            ? 'bg-blue-500'
                                            : 'bg-gray-700'
                                            }`}
                                    />
                                ))}
                            </div>
                        </div>
                    ) : status === 'stopping' ? (
                        <div className="absolute inset-0 flex flex-col items-center justify-center p-6 bg-gradient-to-b from-gray-900 via-gray-900 to-black">
                            <div className="mb-6">
                                <svg className="w-16 h-16 text-red-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M18.364 18.364A9 9 0 005.636 5.636m12.728 12.728A9 9 0 015.636 5.636m12.728 12.728L5.636 5.636" />
                                </svg>
                            </div>
                            <p className="text-red-400 text-lg font-medium text-center mb-2">
                                Shutting down...
                            </p>
                        </div>
                    ) : (
                        <iframe
                            src={streamUrl || ''}
                            className="w-full h-full border-0"
                            allow="autoplay; fullscreen; microphone; clipboard-read; clipboard-write"
                            title={`Chrome Browser - ${vmName}`}
                            tabIndex={0}
                            onLoad={(e) => {
                                (e.target as HTMLIFrameElement).focus();
                            }}
                        />
                    )}
                </div>
            </div>

            <p className="mt-4 text-sm font-mono text-gray-500">{vmName}</p>
        </div>
    );
}
