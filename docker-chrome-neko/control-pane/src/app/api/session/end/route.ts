import { NextRequest, NextResponse } from 'next/server';
import { deleteVM } from '@/lib/compute';

export async function POST(request: NextRequest) {
  try {
    const { vmName } = await request.json();
    
    if (!vmName) {
      return NextResponse.json(
        { error: 'vmName is required' },
        { status: 400 }
      );
    }

    console.log('[API] Ending session:', vmName);
    await deleteVM(vmName);

    return NextResponse.json({
      success: true,
      message: `Session ${vmName} terminated`,
    });
  } catch (error) {
    console.error('[API] Error ending session:', error);
    return NextResponse.json(
      { error: error instanceof Error ? error.message : 'Failed to end session' },
      { status: 500 }
    );
  }
}
