import React from 'react';

interface VM {
  id: string;
  name: string;
  state: string;
  cpu_count: number;
  memory_mb: number;
  display_mode: string;
}

interface VMListProps {
  vms: VM[];
  selectedVm: VM | null;
  onSelect: (vm: VM) => void;
  onStart: (id: string) => void;
  onStop: (id: string) => void;
  onPause: (id: string) => void;
  loading: boolean;
}

export default function VMList({
  vms,
  selectedVm,
  onSelect,
  onStart,
  onStop,
  onPause,
  loading,
}: VMListProps) {
  const isRunning = (state: any) =>
    typeof state === 'string' && state === 'Running';
  const isPaused = (state: any) =>
    typeof state === 'string' && state === 'Paused';

  return (
    <div className="grid grid-cols-1 gap-4">
      {vms.map((vm) => (
        <div
          key={vm.id}
          onClick={() => onSelect(vm)}
          className={`p-4 rounded-lg border-2 cursor-pointer transition ${
            selectedVm?.id === vm.id
              ? 'border-blue-500 bg-blue-50'
              : 'border-gray-200 bg-white hover:border-gray-300'
          }`}
        >
          <div className="flex justify-between items-start">
            <div className="flex-1">
              <h3 className="font-semibold text-lg text-gray-800">
                {vm.name}
              </h3>
              <p className="text-sm text-gray-500">ID: {vm.id}</p>
              <div className="mt-2 space-y-1">
                <p className="text-sm">
                  <span className="text-gray-600">Status:</span>{' '}
                  <span
                    className={`font-medium ${
                      isRunning(vm.state)
                        ? 'text-green-600'
                        : isPaused(vm.state)
                          ? 'text-yellow-600'
                          : 'text-red-600'
                    }`}
                  >
                    {typeof vm.state === 'string' ? vm.state : 'Error'}
                  </span>
                </p>
                <p className="text-sm text-gray-600">
                  CPU: {vm.cpu_count} cores | RAM: {vm.memory_mb} MB
                </p>
                <p className="text-sm text-gray-600">
                  Display: {vm.display_mode}
                </p>
              </div>
            </div>

            {/* Control Buttons */}
            <div className="ml-4 space-y-2 flex flex-col">
              {!isRunning(vm.state) && (
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    onStart(vm.id);
                  }}
                  disabled={loading}
                  className="px-3 py-1 bg-green-500 text-white text-sm rounded hover:bg-green-600 disabled:opacity-50"
                >
                  Start
                </button>
              )}
              {isRunning(vm.state) && (
                <>
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      onPause(vm.id);
                    }}
                    disabled={loading}
                    className="px-3 py-1 bg-yellow-500 text-white text-sm rounded hover:bg-yellow-600 disabled:opacity-50"
                  >
                    Pause
                  </button>
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      onStop(vm.id);
                    }}
                    disabled={loading}
                    className="px-3 py-1 bg-red-500 text-white text-sm rounded hover:bg-red-600 disabled:opacity-50"
                  >
                    Stop
                  </button>
                </>
              )}
              {isPaused(vm.state) && (
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    onStart(vm.id);
                  }}
                  disabled={loading}
                  className="px-3 py-1 bg-blue-500 text-white text-sm rounded hover:bg-blue-600 disabled:opacity-50"
                >
                  Resume
                </button>
              )}
            </div>
          </div>
        </div>
      ))}

      {vms.length === 0 && (
        <div className="text-center py-12 bg-white rounded-lg border border-gray-200">
          <p className="text-gray-500">No virtual machines found</p>
        </div>
      )}
    </div>
  );
}
