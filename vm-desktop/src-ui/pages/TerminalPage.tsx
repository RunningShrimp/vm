import React, { useState, useRef, useEffect } from 'react';
import { Card, Input, Button } from '../atoms';

interface TerminalSession {
  id: string;
  vmName: string;
  isConnected: boolean;
  lastMessage: string;
}

interface TerminalLine {
  id: string;
  content: string;
  isCommand: boolean;
  timestamp: number;
}

export const TerminalPage: React.FC = () => {
  const [sessions, setSessions] = useState<TerminalSession[]>([
    { id: 'vm1', vmName: 'Ubuntu-20.04', isConnected: true, lastMessage: 'root@ubuntu:~#' },
    { id: 'vm2', vmName: 'CentOS-8', isConnected: false, lastMessage: 'Connection closed' },
    { id: 'vm3', vmName: 'Windows-Server-2022', isConnected: true, lastMessage: 'PS C:\\Users\\Admin>' },
  ]);

  const [activeSessionId, setActiveSessionId] = useState('vm1');
  const [output, setOutput] = useState<TerminalLine[]>([
    { id: '1', content: 'Welcome to VM Terminal', isCommand: false, timestamp: Date.now() },
    { id: '2', content: 'root@ubuntu:~#', isCommand: false, timestamp: Date.now() + 100 },
  ]);
  const [command, setCommand] = useState('');
  const terminalRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  const activeSession = sessions.find(s => s.id === activeSessionId);

  useEffect(() => {
    if (terminalRef.current) {
      terminalRef.current.scrollTop = terminalRef.current.scrollHeight;
    }
  }, [output]);

  const handleCommand = () => {
    if (!command.trim() || !activeSession?.isConnected) return;

    const newLine: TerminalLine = {
      id: Date.now().toString(),
      content: command,
      isCommand: true,
      timestamp: Date.now(),
    };

    setOutput([...output, newLine]);

    // Simulate command response
    setTimeout(() => {
      let response = '';
      const cmd = command.toLowerCase().trim();

      if (cmd === 'ls' || cmd === 'dir') {
        response = `Desktop  Documents  Downloads  Music  Pictures  Public  Templates  Videos`;
      } else if (cmd === 'pwd') {
        response = '/home/user';
      } else if (cmd === 'whoami') {
        response = 'user';
      } else if (cmd === 'date') {
        response = new Date().toString();
      } else if (cmd === 'uname -a') {
        response = 'Linux ubuntu 5.15.0-56-generic #62-Ubuntu SMP Tue Nov 22 21:24:11 UTC 2022 x86_64 x86_64 x86_64 GNU/Linux';
      } else if (cmd === 'free -h') {
        response = `              total        used        free      shared  buff/cache   available
Mem:           7.8G        3.2G        2.1G        512M        2.5G        4.6G
Swap:          2.0G        1.0G        1.0G`;
      } else if (cmd === 'df -h') {
        response = `Filesystem      Size  Used Avail Use% Mounted on
/dev/sda1       50G   25G   25G  50% /
tmpfs          3.9G  0    3.9G   0% /dev/shm`;
      } else if (cmd === 'top -n 1' || cmd === 'ps aux') {
        response = `PID   USER      PR  NI    VIRT    RES    SHR S  %CPU %MEM     TIME+ COMMAND
  1   root      20   0  101436   4840   3640 S   0.0  0.1   0:00.10 init
100   root      20   0  644092  65432  45324 S   2.5  0.8   0:15.43 firefox
205   user      20   0  512000  32156  22540 S   1.2  0.4   0:08.21 code`;
      } else if (cmd === 'help' || cmd === '?') {
        response = `Available commands:
ls, dir          - List directory contents
pwd              - Print working directory
whoami           - Display current user
date             - Show current date and time
uname -a         - Show system information
free -h          - Display memory usage
df -h            - Show disk space usage
top -n 1         - Show process info
ps aux           - List all processes
help, ?          - Show this help message`;
      } else {
        response = `Command not found: ${command}. Type 'help' for available commands.`;
      }

      const responseLine: TerminalLine = {
        id: (Date.now() + 1).toString(),
        content: response,
        isCommand: false,
        timestamp: Date.now(),
      };

      const promptLine: TerminalLine = {
        id: (Date.now() + 2).toString(),
        content: 'root@ubuntu:~#',
        isCommand: false,
        timestamp: Date.now(),
      };

      setOutput(prev => [...prev, responseLine, promptLine]);
    }, 300);

    setCommand('');
  };

  const handleSwitchSession = (sessionId: string) => {
    setActiveSessionId(sessionId);
    setOutput([
      { id: '1', content: `Connected to ${sessions.find(s => s.id === sessionId)?.vmName}`, isCommand: false, timestamp: Date.now() },
      { id: '2', content: 'root@vm:~#', isCommand: false, timestamp: Date.now() + 100 },
    ]);
  };

  const handleClearTerminal = () => {
    setOutput([
      { id: '1', content: 'Terminal cleared', isCommand: false, timestamp: Date.now() },
      { id: '2', content: 'root@ubuntu:~#', isCommand: false, timestamp: Date.now() + 100 },
    ]);
  };

  const handleCloseSession = (sessionId: string) => {
    const filtered = sessions.filter(s => s.id !== sessionId);
    setSessions(filtered);
    if (activeSessionId === sessionId && filtered.length > 0) {
      setActiveSessionId(filtered[0].id);
    }
  };

  return (
    <div className="flex h-full gap-4 p-4">
      {/* Sessions Panel */}
      <div className="w-48 flex flex-col gap-2">
        <h3 className="text-sm font-semibold text-gray-700 mb-2">Active Sessions</h3>
        <div className="space-y-2">
          {sessions.map(session => (
            <div
              key={session.id}
              className={`p-2 rounded cursor-pointer transition ${
                activeSessionId === session.id
                  ? 'bg-blue-500 text-white'
                  : 'bg-gray-200 text-gray-800 hover:bg-gray-300'
              }`}
            >
              <div
                className="flex items-center justify-between gap-2"
                onClick={() => handleSwitchSession(session.id)}
              >
                <div className="flex-1 min-w-0">
                  <div className="text-xs font-semibold truncate">{session.vmName}</div>
                  <div className={`text-xs ${session.isConnected ? 'text-green-600' : 'text-red-600'}`}>
                    {session.isConnected ? '●' : '○'} {session.isConnected ? 'Connected' : 'Offline'}
                  </div>
                </div>
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    handleCloseSession(session.id);
                  }}
                  className="text-xs px-1.5 py-0.5 rounded bg-red-500 hover:bg-red-600 text-white"
                >
                  ✕
                </button>
              </div>
            </div>
          ))}
        </div>
      </div>

      {/* Terminal Area */}
      <div className="flex-1 flex flex-col gap-2">
        {/* Terminal Header */}
        <Card className="p-3 bg-gray-900 text-green-400 flex items-center justify-between">
          <div className="flex items-center gap-2">
            <div className="text-xs">
              {activeSession?.vmName} {activeSession?.isConnected ? '(Connected)' : '(Offline)'}
            </div>
            <div className="text-xs text-gray-500">|</div>
            <div className="text-xs">root@ubuntu</div>
          </div>
          <Button
            onClick={handleClearTerminal}
            className="text-xs px-2 py-1"
          >
            Clear
          </Button>
        </Card>

        {/* Terminal Output */}
        <Card
          ref={terminalRef}
          className="flex-1 overflow-y-auto bg-gray-900 text-green-400 font-mono text-sm p-4 rounded"
        >
          {output.map(line => (
            <div
              key={line.id}
              className={line.isCommand ? 'text-white font-semibold' : 'text-green-400'}
            >
              {line.isCommand && <span className="text-blue-400">$ </span>}
              {line.content}
            </div>
          ))}
        </Card>

        {/* Terminal Input */}
        <div className="flex gap-2 bg-gray-900 p-3 rounded">
          <span className="text-green-400 font-mono text-sm flex-shrink-0">$</span>
          <Input
            ref={inputRef}
            type="text"
            value={command}
            onChange={(e) => setCommand(e.target.value)}
            onKeyPress={(e) => {
              if (e.key === 'Enter') {
                handleCommand();
              }
            }}
            placeholder="Enter command..."
            disabled={!activeSession?.isConnected}
            className="flex-1 bg-gray-800 text-green-400 font-mono text-sm border-0"
          />
          <Button
            onClick={handleCommand}
            disabled={!activeSession?.isConnected}
            className="px-4"
          >
            Execute
          </Button>
        </div>

        {/* Terminal Info */}
        <div className="text-xs text-gray-500 flex justify-between">
          <div>{output.length} lines output</div>
          <div>Terminal Mode • UTF-8</div>
        </div>
      </div>
    </div>
  );
};
