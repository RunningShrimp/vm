import React from 'react';

interface TerminalProps {
  vmId: string;
  vmName: string;
}

export default function Terminal({ vmId, vmName }: TerminalProps) {
  const terminalRef = React.useRef<HTMLDivElement>(null);

  React.useEffect(() => {
    // TODO: Initialize xterm.js terminal
    // 1. Create a new Terminal instance
    // 2. Open it in the terminalRef div
    // 3. Connect to the VM's serial port via IPC
    // 4. Send/receive data from the VM
  }, [vmId]);

  return (
    <div className="h-full bg-black rounded-lg p-4">
      <div className="flex items-center justify-between mb-4">
        <h3 className="text-white font-semibold">
          Terminal - {vmName}
        </h3>
        <button
          onClick={() => {
            if (terminalRef.current) {
              terminalRef.current.innerHTML = '';
            }
          }}
          className="px-3 py-1 bg-red-600 text-white text-sm rounded hover:bg-red-700"
        >
          Clear
        </button>
      </div>
      <div
        ref={terminalRef}
        className="font-mono text-sm text-green-400 h-[calc(100%-40px)] overflow-hidden"
        style={{ backgroundColor: '#000000' }}
      />
    </div>
  );
}
