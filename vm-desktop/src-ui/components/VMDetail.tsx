import React from 'react';

interface VM {
  id: string;
  name: string;
  state: string;
  cpu_count: number;
  memory_mb: number;
  display_mode: string;
}

interface VMDetailProps {
  vm?: VM;
}

export default function VMDetail({ vm }: VMDetailProps) {
  if (!vm) {
    return (
      <div className="bg-white rounded-lg shadow p-6">
        <p className="text-gray-500">Select a VM to view details</p>
      </div>
    );
  }

  return (
    <div className="bg-white rounded-lg shadow p-6">
      <h2 className="text-2xl font-bold mb-4">{vm.name}</h2>
      
      <div className="grid grid-cols-2 gap-4">
        <div>
          <label className="text-gray-600 text-sm">VM ID</label>
          <p className="text-gray-800 font-mono">{vm.id}</p>
        </div>

        <div>
          <label className="text-gray-600 text-sm">Status</label>
          <p className={`font-medium ${
            typeof vm.state === 'string' && vm.state === 'Running'
              ? 'text-green-600'
              : typeof vm.state === 'string' && vm.state === 'Paused'
                ? 'text-yellow-600'
                : 'text-red-600'
          }`}>
            {typeof vm.state === 'string' ? vm.state : 'Error'}
          </p>
        </div>

        <div>
          <label className="text-gray-600 text-sm">CPU Cores</label>
          <p className="text-gray-800">{vm.cpu_count}</p>
        </div>

        <div>
          <label className="text-gray-600 text-sm">Memory</label>
          <p className="text-gray-800">{vm.memory_mb} MB</p>
        </div>

        <div>
          <label className="text-gray-600 text-sm">Display Mode</label>
          <p className="text-gray-800">{vm.display_mode}</p>
        </div>
      </div>
    </div>
  );
}
