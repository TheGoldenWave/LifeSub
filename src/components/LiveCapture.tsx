interface LiveCaptureProps {
  onNotice: (msg: string) => void
}

export function LiveCapture({ onNotice: _onNotice }: LiveCaptureProps) {
  return <main className="page-placeholder"><h1>Live Capture</h1><p>实时录音与转写</p></main>
}