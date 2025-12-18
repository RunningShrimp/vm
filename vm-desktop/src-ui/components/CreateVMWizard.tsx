import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/tauri';
import { XMarkIcon, ChevronRightIcon, ChevronDownIcon, CheckIcon } from '@heroicons/react/24/outline';

interface VmConfig {
  id: string;
  name: string;
  cpu_count: number;
  memory_mb: number;
  disk_gb: number;
  display_mode: 'GUI' | 'Terminal';
  os_type: 'Ubuntu' | 'Debian' | 'Windows' | 'CentOS' | 'Other';
}

interface CreateVMWizardProps {
  onComplete?: () => void;
  onCancel?: () => void;
}

interface WizardStep {
  id: string;
  title: string;
  description: string;
}

export const CreateVMWizard: React.FC<CreateVMWizardProps> = ({ onComplete, onCancel }) => {
  const [currentStep, setCurrentStep] = useState(0);
  const [config, setConfig] = useState<VmConfig>({
    id: '',
    name: '',
    cpu_count: 2,
    memory_mb: 2048,
    disk_gb: 20,
    display_mode: 'GUI',
    os_type: 'Ubuntu',
  });
  const [kernelPath, setKernelPath] = useState('');
  const [startPc, setStartPc] = useState('0x80000000');
  const [isCreating, setIsCreating] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [expandedSections, setExpandedSections] = useState<Record<string, boolean>>({});

  const steps: WizardStep[] = [
    {
      id: 'basic',
      title: 'Basic Configuration',
      description: 'Set up the basic VM parameters',
    },
    {
      id: 'resources',
      title: 'Resource Allocation',
      description: 'Configure CPU, memory, and disk resources',
    },
    {
      id: 'advanced',
      title: 'Advanced Settings',
      description: 'Configure kernel path and execution parameters',
    },
    {
      id: 'review',
      title: 'Review & Create',
      description: 'Review your configuration and create the VM',
    },
  ];

  const osOptions = [
    { value: 'Ubuntu', label: 'Ubuntu', icon: '🟧' },
    { value: 'Debian', label: 'Debian', icon: '🔷' },
    { value: 'Windows', label: 'Windows', icon: '🪟' },
    { value: 'CentOS', label: 'CentOS', icon: '🔴' },
    { value: 'Other', label: 'Other', icon: '📦' },
  ];

  const cpuPresets = [
    { label: '1 vCPU', value: 1, description: 'Light workloads' },
    { label: '2 vCPUs', value: 2, description: 'General purpose' },
    { label: '4 vCPUs', value: 4, description: 'Compute intensive' },
    { label: '8 vCPUs', value: 8, description: 'High performance' },
  ];

  const memoryPresets = [
    { label: '1 GB', value: 1024, description: 'Minimum' },
    { label: '2 GB', value: 2048, description: 'Recommended' },
    { label: '4 GB', value: 4096, description: 'Better performance' },
    { label: '8 GB', value: 8192, description: 'High performance' },
    { label: '16 GB', value: 16384, description: 'Maximum' },
  ];

  const diskPresets = [
    { label: '10 GB', value: 10, description: 'Minimal' },
    { label: '20 GB', value: 20, description: 'Standard' },
    { label: '50 GB', value: 50, description: 'Large' },
    { label: '100 GB', value: 100, description: 'Very large' },
  ];

  const toggleSection = (section: string) => {
    setExpandedSections(prev => ({
      ...prev,
      [section]: !prev[section],
    }));
  };

  const validateStep = (step: number): boolean => {
    switch (step) {
      case 0: // Basic
        return config.name.trim() !== '' && config.id.trim() !== '';
      case 1: // Resources
        return config.cpu_count > 0 && config.memory_mb > 0 && config.disk_gb > 0;
      case 2: // Advanced
        return true; // Advanced settings are optional
      case 3: // Review
        return true; // All validation done in previous steps
      default:
        return false;
    }
  };

  const nextStep = () => {
    if (validateStep(currentStep) && currentStep < steps.length - 1) {
      setCurrentStep(currentStep + 1);
    }
  };

  const prevStep = () => {
    if (currentStep > 0) {
      setCurrentStep(currentStep - 1);
    }
  };

  const handleCreate = async () => {
    if (!validateStep(currentStep)) return;

    setIsCreating(true);
    setError(null);

    try {
      // Create VM with basic config
      const vm = await invoke<VmConfig>('create_vm', { config });
      
      // Set kernel path if provided
      if (kernelPath.trim()) {
        await invoke('set_kernel_path', { id: vm.id, path: kernelPath });
      }
      
      // Set start PC if provided
      if (startPc.trim()) {
        await invoke('set_start_pc', { id: vm.id, startPc });
      }

      onComplete?.();
      
      // Reset form
      setConfig({
        id: '',
        name: '',
        cpu_count: 2,
        memory_mb: 2048,
        disk_gb: 20,
        display_mode: 'GUI',
        os_type: 'Ubuntu',
      });
      setKernelPath('');
      setStartPc('0x80000000');
      setCurrentStep(0);
    } catch (err) {
      setError(`Failed to create VM: ${err}`);
    } finally {
      setIsCreating(false);
    }
  };

  const renderStepContent = () => {
    switch (currentStep) {
      case 0: // Basic Configuration
        return (
          <div className="space-y-6">
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-2">
                VM Name
              </label>
              <input
                type="text"
                value={config.name}
                onChange={(e) => setConfig({ ...config, name: e.target.value })}
                className="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
                placeholder="Enter VM name"
              />
            </div>

            <div>
              <label className="block text-sm font-medium text-gray-700 mb-2">
                VM ID
              </label>
              <input
                type="text"
                value={config.id}
                onChange={(e) => setConfig({ ...config, id: e.target.value.replace(/[^a-zA-Z0-9-_]/g, '') })}
                className="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
                placeholder="Enter unique VM ID"
              />
              <p className="mt-1 text-sm text-gray-500">
                Only alphanumeric characters, hyphens, and underscores are allowed
              </p>
            </div>

            <div>
              <label className="block text-sm font-medium text-gray-700 mb-2">
                Operating System
              </label>
              <div className="grid grid-cols-2 md:grid-cols-3 gap-3">
                {osOptions.map((os) => (
                  <button
                    key={os.value}
                    onClick={() => setConfig({ ...config, os_type: os.value as any })}
                    className={`p-3 border rounded-lg flex flex-col items-center space-y-2 transition-colors ${
                      config.os_type === os.value
                        ? 'border-blue-500 bg-blue-50 text-blue-700'
                        : 'border-gray-300 hover:border-gray-400'
                    }`}
                  >
                    <span className="text-2xl">{os.icon}</span>
                    <span className="text-sm font-medium">{os.label}</span>
                  </button>
                ))}
              </div>
            </div>

            <div>
              <label className="block text-sm font-medium text-gray-700 mb-2">
                Display Mode
              </label>
              <div className="grid grid-cols-2 gap-3">
                <button
                  onClick={() => setConfig({ ...config, display_mode: 'GUI' })}
                  className={`p-3 border rounded-lg flex items-center justify-center space-x-2 transition-colors ${
                    config.display_mode === 'GUI'
                      ? 'border-blue-500 bg-blue-50 text-blue-700'
                      : 'border-gray-300 hover:border-gray-400'
                  }`}
                >
                  <span>🖥️</span>
                  <span className="font-medium">GUI</span>
                </button>
                <button
                  onClick={() => setConfig({ ...config, display_mode: 'Terminal' })}
                  className={`p-3 border rounded-lg flex items-center justify-center space-x-2 transition-colors ${
                    config.display_mode === 'Terminal'
                      ? 'border-blue-500 bg-blue-50 text-blue-700'
                      : 'border-gray-300 hover:border-gray-400'
                  }`}
                >
                  <span>💻</span>
                  <span className="font-medium">Terminal</span>
                </button>
              </div>
            </div>
          </div>
        );

      case 1: // Resource Allocation
        return (
          <div className="space-y-6">
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-2">
                CPU Cores
              </label>
              <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
                {cpuPresets.map((preset) => (
                  <button
                    key={preset.value}
                    onClick={() => setConfig({ ...config, cpu_count: preset.value })}
                    className={`p-3 border rounded-lg text-center transition-colors ${
                      config.cpu_count === preset.value
                        ? 'border-blue-500 bg-blue-50 text-blue-700'
                        : 'border-gray-300 hover:border-gray-400'
                    }`}
                  >
                    <div className="font-medium">{preset.label}</div>
                    <div className="text-xs text-gray-500">{preset.description}</div>
                  </button>
                ))}
              </div>
              <div className="mt-3">
                <label className="block text-xs text-gray-500 mb-1">Custom: {config.cpu_count} vCPUs</label>
                <input
                  type="range"
                  min="1"
                  max="16"
                  value={config.cpu_count}
                  onChange={(e) => setConfig({ ...config, cpu_count: parseInt(e.target.value) })}
                  className="w-full"
                />
              </div>
            </div>

            <div>
              <label className="block text-sm font-medium text-gray-700 mb-2">
                Memory
              </label>
              <div className="grid grid-cols-2 md:grid-cols-5 gap-3">
                {memoryPresets.map((preset) => (
                  <button
                    key={preset.value}
                    onClick={() => setConfig({ ...config, memory_mb: preset.value })}
                    className={`p-3 border rounded-lg text-center transition-colors ${
                      config.memory_mb === preset.value
                        ? 'border-blue-500 bg-blue-50 text-blue-700'
                        : 'border-gray-300 hover:border-gray-400'
                    }`}
                  >
                    <div className="font-medium">{preset.label}</div>
                    <div className="text-xs text-gray-500">{preset.description}</div>
                  </button>
                ))}
              </div>
              <div className="mt-3">
                <label className="block text-xs text-gray-500 mb-1">Custom: {config.memory_mb} MB</label>
                <input
                  type="range"
                  min="512"
                  max="32768"
                  step="512"
                  value={config.memory_mb}
                  onChange={(e) => setConfig({ ...config, memory_mb: parseInt(e.target.value) })}
                  className="w-full"
                />
              </div>
            </div>

            <div>
              <label className="block text-sm font-medium text-gray-700 mb-2">
                Disk Size
              </label>
              <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
                {diskPresets.map((preset) => (
                  <button
                    key={preset.value}
                    onClick={() => setConfig({ ...config, disk_gb: preset.value })}
                    className={`p-3 border rounded-lg text-center transition-colors ${
                      config.disk_gb === preset.value
                        ? 'border-blue-500 bg-blue-50 text-blue-700'
                        : 'border-gray-300 hover:border-gray-400'
                    }`}
                  >
                    <div className="font-medium">{preset.label}</div>
                    <div className="text-xs text-gray-500">{preset.description}</div>
                  </button>
                ))}
              </div>
              <div className="mt-3">
                <label className="block text-xs text-gray-500 mb-1">Custom: {config.disk_gb} GB</label>
                <input
                  type="range"
                  min="5"
                  max="1000"
                  step="5"
                  value={config.disk_gb}
                  onChange={(e) => setConfig({ ...config, disk_gb: parseInt(e.target.value) })}
                  className="w-full"
                />
              </div>
            </div>
          </div>
        );

      case 2: // Advanced Settings
        return (
          <div className="space-y-6">
            <div>
              <button
                onClick={() => toggleSection('kernel')}
                className="w-full flex items-center justify-between p-3 border border-gray-300 rounded-lg hover:bg-gray-50"
              >
                <span className="font-medium">Kernel Configuration</span>
                {expandedSections.kernel ? (
                  <ChevronDownIcon className="h-5 w-5" />
                ) : (
                  <ChevronRightIcon className="h-5 w-5" />
                )}
              </button>
              
              {expandedSections.kernel && (
                <div className="mt-3 space-y-3 p-3 bg-gray-50 rounded-lg">
                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-2">
                      Kernel Path
                    </label>
                    <input
                      type="text"
                      value={kernelPath}
                      onChange={(e) => setKernelPath(e.target.value)}
                      className="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
                      placeholder="/path/to/kernel"
                    />
                    <p className="mt-1 text-sm text-gray-500">
                      Path to the kernel image file (optional)
                    </p>
                  </div>

                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-2">
                      Start PC (Program Counter)
                    </label>
                    <input
                      type="text"
                      value={startPc}
                      onChange={(e) => setStartPc(e.target.value)}
                      className="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
                      placeholder="0x80000000"
                    />
                    <p className="mt-1 text-sm text-gray-500">
                      Memory address where execution should begin (hexadecimal)
                    </p>
                  </div>
                </div>
              )}
            </div>

            <div>
              <button
                onClick={() => toggleSection('performance')}
                className="w-full flex items-center justify-between p-3 border border-gray-300 rounded-lg hover:bg-gray-50"
              >
                <span className="font-medium">Performance Options</span>
                {expandedSections.performance ? (
                  <ChevronDownIcon className="h-5 w-5" />
                ) : (
                  <ChevronRightIcon className="h-5 w-5" />
                )}
              </button>
              
              {expandedSections.performance && (
                <div className="mt-3 space-y-3 p-3 bg-gray-50 rounded-lg">
                  <div className="text-sm text-gray-600">
                    Performance options will be available in a future release. This will include:
                    <ul className="mt-2 ml-4 list-disc space-y-1">
                      <li>JIT compilation settings</li>
                      <li>Memory optimization options</li>
                      <li>CPU affinity and NUMA settings</li>
                      <li>I/O optimization parameters</li>
                    </ul>
                  </div>
                </div>
              )}
            </div>

            <div>
              <button
                onClick={() => toggleSection('network')}
                className="w-full flex items-center justify-between p-3 border border-gray-300 rounded-lg hover:bg-gray-50"
              >
                <span className="font-medium">Network Configuration</span>
                {expandedSections.network ? (
                  <ChevronDownIcon className="h-5 w-5" />
                ) : (
                  <ChevronRightIcon className="h-5 w-5" />
                )}
              </button>
              
              {expandedSections.network && (
                <div className="mt-3 space-y-3 p-3 bg-gray-50 rounded-lg">
                  <div className="text-sm text-gray-600">
                    Network configuration will be available in a future release. This will include:
                    <ul className="mt-2 ml-4 list-disc space-y-1">
                      <li>Network interface configuration</li>
                      <li>Port forwarding rules</li>
                      <li>Network QoS settings</li>
                      <li>Virtual network setup</li>
                    </ul>
                  </div>
                </div>
              )}
            </div>
          </div>
        );

      case 3: // Review & Create
        return (
          <div className="space-y-6">
            <div className="bg-gray-50 p-6 rounded-lg">
              <h3 className="text-lg font-medium text-gray-900 mb-4">VM Configuration Summary</h3>
              
              <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                <div>
                  <h4 className="font-medium text-gray-700 mb-2">Basic Information</h4>
                  <dl className="space-y-1">
                    <div className="flex justify-between">
                      <dt className="text-sm text-gray-500">Name:</dt>
                      <dd className="text-sm font-medium">{config.name}</dd>
                    </div>
                    <div className="flex justify-between">
                      <dt className="text-sm text-gray-500">ID:</dt>
                      <dd className="text-sm font-medium">{config.id}</dd>
                    </div>
                    <div className="flex justify-between">
                      <dt className="text-sm text-gray-500">OS:</dt>
                      <dd className="text-sm font-medium">{config.os_type}</dd>
                    </div>
                    <div className="flex justify-between">
                      <dt className="text-sm text-gray-500">Display:</dt>
                      <dd className="text-sm font-medium">{config.display_mode}</dd>
                    </div>
                  </dl>
                </div>

                <div>
                  <h4 className="font-medium text-gray-700 mb-2">Resources</h4>
                  <dl className="space-y-1">
                    <div className="flex justify-between">
                      <dt className="text-sm text-gray-500">CPU:</dt>
                      <dd className="text-sm font-medium">{config.cpu_count} vCPUs</dd>
                    </div>
                    <div className="flex justify-between">
                      <dt className="text-sm text-gray-500">Memory:</dt>
                      <dd className="text-sm font-medium">{config.memory_mb} MB</dd>
                    </div>
                    <div className="flex justify-between">
                      <dt className="text-sm text-gray-500">Disk:</dt>
                      <dd className="text-sm font-medium">{config.disk_gb} GB</dd>
                    </div>
                  </dl>
                </div>
              </div>

              {kernelPath && (
                <div className="mt-4 pt-4 border-t border-gray-200">
                  <h4 className="font-medium text-gray-700 mb-2">Advanced Settings</h4>
                  <dl className="space-y-1">
                    <div className="flex justify-between">
                      <dt className="text-sm text-gray-500">Kernel Path:</dt>
                      <dd className="text-sm font-medium">{kernelPath}</dd>
                    </div>
                    <div className="flex justify-between">
                      <dt className="text-sm text-gray-500">Start PC:</dt>
                      <dd className="text-sm font-medium">{startPc}</dd>
                    </div>
                  </dl>
                </div>
              )}
            </div>

            {error && (
              <div className="bg-red-50 border border-red-200 rounded-md p-4">
                <div className="text-red-800">{error}</div>
              </div>
            )}
          </div>
        );

      default:
        return null;
    }
  };

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 bg-gray-600 bg-opacity-50 overflow-y-auto h-full w-full z-50">
      <div className="relative top-20 mx-auto p-5 border w-11/12 md:w-3/4 lg:w-1/2 shadow-lg rounded-md bg-white">
        {/* Header */}
        <div className="flex items-center justify-between mb-6">
          <h3 className="text-lg font-medium text-gray-900">Create New Virtual Machine</h3>
          <button
            onClick={onCancel}
            className="text-gray-400 hover:text-gray-500"
          >
            <XMarkIcon className="h-6 w-6" />
          </button>
        </div>

        {/* Progress Steps */}
        <div className="mb-8">
          <div className="flex items-center">
            {steps.map((step, index) => (
              <div key={step.id} className="flex items-center">
                <div
                  className={`flex items-center justify-center w-8 h-8 rounded-full text-sm font-medium ${
                    index <= currentStep
                      ? 'bg-blue-600 text-white'
                      : 'bg-gray-200 text-gray-500'
                  }`}
                >
                  {index < currentStep ? (
                    <CheckIcon className="h-5 w-5" />
                  ) : (
                    index + 1
                  )}
                </div>
                <div
                  className={`flex-1 h-1 mx-2 ${
                    index < currentStep ? 'bg-blue-600' : 'bg-gray-200'
                  }`}
                />
                <div className="text-center">
                  <div className="text-sm font-medium text-gray-900">{step.title}</div>
                  <div className="text-xs text-gray-500">{step.description}</div>
                </div>
              </div>
            ))}
          </div>
        </div>

        {/* Step Content */}
        <div className="mb-8">
          {renderStepContent()}
        </div>

        {/* Navigation */}
        <div className="flex justify-between">
          <button
            onClick={prevStep}
            disabled={currentStep === 0}
            className={`px-4 py-2 rounded-md text-sm font-medium ${
              currentStep === 0
                ? 'bg-gray-100 text-gray-400 cursor-not-allowed'
                : 'bg-white border border-gray-300 text-gray-700 hover:bg-gray-50'
            }`}
          >
            Previous
          </button>

          <div className="flex space-x-2">
            {currentStep < steps.length - 1 ? (
              <button
                onClick={nextStep}
                disabled={!validateStep(currentStep)}
                className={`px-4 py-2 rounded-md text-sm font-medium ${
                  validateStep(currentStep)
                    ? 'bg-blue-600 text-white hover:bg-blue-700'
                    : 'bg-gray-100 text-gray-400 cursor-not-allowed'
                }`}
              >
                Next
              </button>
            ) : (
              <button
                onClick={handleCreate}
                disabled={isCreating || !validateStep(currentStep)}
                className={`px-4 py-2 rounded-md text-sm font-medium ${
                  isCreating || !validateStep(currentStep)
                    ? 'bg-gray-100 text-gray-400 cursor-not-allowed'
                    : 'bg-blue-600 text-white hover:bg-blue-700'
                }`}
              >
                {isCreating ? 'Creating...' : 'Create VM'}
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
};