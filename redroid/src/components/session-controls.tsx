'use client';

interface SessionControlsProps {
    isLoading: boolean;
    hasActiveSession: boolean;
    sessionStatus: 'starting' | 'running' | 'ended' | 'error' | null;
    onStart: () => void;
    onEnd: () => void;
}

export function SessionControls({
    isLoading,
    hasActiveSession,
    sessionStatus,
    onStart,
    onEnd,
}: SessionControlsProps) {
    const canStart = !hasActiveSession || sessionStatus === 'ended' || sessionStatus === 'error';
    const canEnd = hasActiveSession && (sessionStatus === 'starting' || sessionStatus === 'running');

    return (
        <div className="flex gap-4">
            <button
                onClick={onStart}
                disabled={!canStart || isLoading}
                className={`
          flex-1 py-4 px-6 rounded-xl font-semibold text-lg transition-all duration-200
          ${canStart && !isLoading
                        ? 'bg-gradient-to-r from-blue-600 to-purple-600 hover:from-blue-500 hover:to-purple-500 text-white shadow-lg shadow-blue-900/30 hover:shadow-xl hover:shadow-blue-900/40 hover:-translate-y-0.5'
                        : 'bg-gray-700 text-gray-400 cursor-not-allowed'
                    }
        `}
            >
                {isLoading && sessionStatus === null ? (
                    <span className="flex items-center justify-center gap-2">
                        <svg className="animate-spin h-5 w-5" fill="none" viewBox="0 0 24 24">
                            <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                            <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
                        </svg>
                        Starting...
                    </span>
                ) : (
                    <span className="flex items-center justify-center gap-2">
                        <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M14.752 11.168l-3.197-2.132A1 1 0 0010 9.87v4.263a1 1 0 001.555.832l3.197-2.132a1 1 0 000-1.664z" />
                            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                        </svg>
                        Start Session
                    </span>
                )}
            </button>

            <button
                onClick={onEnd}
                disabled={!canEnd || isLoading}
                className={`
          flex-1 py-4 px-6 rounded-xl font-semibold text-lg transition-all duration-200
          ${canEnd && !isLoading
                        ? 'bg-gradient-to-r from-red-600 to-orange-600 hover:from-red-500 hover:to-orange-500 text-white shadow-lg shadow-red-900/30 hover:shadow-xl hover:shadow-red-900/40 hover:-translate-y-0.5'
                        : 'bg-gray-700 text-gray-400 cursor-not-allowed'
                    }
        `}
            >
                {isLoading && (sessionStatus === 'starting' || sessionStatus === 'running') ? (
                    <span className="flex items-center justify-center gap-2">
                        <svg className="animate-spin h-5 w-5" fill="none" viewBox="0 0 24 24">
                            <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                            <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
                        </svg>
                        Ending...
                    </span>
                ) : (
                    <span className="flex items-center justify-center gap-2">
                        <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 10a1 1 0 011-1h4a1 1 0 011 1v4a1 1 0 01-1 1h-4a1 1 0 01-1-1v-4z" />
                        </svg>
                        End Session
                    </span>
                )}
            </button>
        </div>
    );
}
