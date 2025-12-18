import React, { useState } from 'react';
import { Button, Input, Badge } from '../atoms';
import { CardLayout, FormLayout } from '../templates';

interface CreateVMStep {
  step: number;
  name: string;
  icon: string;
}

export const CreateVMWizard: React.FC<{ onComplete?: () => void; onCancel?: () => void }> = ({
  onComplete,
  onCancel,
}) => {
  const [currentStep, setCurrentStep] = useState(1);
  const [formData, setFormData] = useState({
    name: '',
    os: 'ubuntu-20.04',
    cpu: 2,
    memory: 2048,
    disk: 20,
    network: 'bridge',
    displayMode: 'GUI',
    autoStart: false,
    enableSnapshot: true,
  });

  const steps: CreateVMStep[] = [
    { step: 1, name: '基本信息', icon: '📝' },
    { step: 2, name: '系统选择', icon: '🖥️' },
    { step: 3, name: '硬件配置', icon: '⚙️' },
    { step: 4, name: '网络设置', icon: '🌐' },
    { step: 5, name: '确认创建', icon: '✓' },
  ];

  const osOptions = [
    { id: 'ubuntu-20.04', name: 'Ubuntu 20.04 LTS', icon: '🐧' },
    { id: 'ubuntu-22.04', name: 'Ubuntu 22.04 LTS', icon: '🐧' },
    { id: 'debian-11', name: 'Debian 11', icon: '🐧' },
    { id: 'centos-8', name: 'CentOS 8', icon: '🎩' },
    { id: 'rhel-8', name: 'Red Hat Enterprise Linux 8', icon: '🎩' },
    { id: 'windows-2019', name: 'Windows Server 2019', icon: '🪟' },
    { id: 'windows-2022', name: 'Windows Server 2022', icon: '🪟' },
  ];

  const handleNext = () => {
    if (currentStep < steps.length) {
      setCurrentStep(currentStep + 1);
    }
  };

  const handlePrev = () => {
    if (currentStep > 1) {
      setCurrentStep(currentStep - 1);
    }
  };

  const handleFieldChange = (field: string, value: any) => {
    setFormData(prev => ({
      ...prev,
      [field]: value,
    }));
  };

  const handleCreate = () => {
    console.log('Creating VM:', formData);
    alert('虚拟机创建中...');
    onComplete?.();
  };

  const renderStep = () => {
    switch (currentStep) {
      case 1:
        return (
          <div className="space-y-6">
            <h3 className="text-lg font-semibold text-gray-900">虚拟机基本信息</h3>
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-2">虚拟机名称</label>
              <Input
                placeholder="例如: my-server, web-vm"
                value={formData.name}
                onChange={e => handleFieldChange('name', e.target.value)}
              />
              <p className="text-xs text-gray-500 mt-1">虚拟机的唯一标识符，只能包含字母、数字和下划线</p>
            </div>
            <div className="grid grid-cols-2 gap-4">
              <div>
                <label className="block text-sm font-medium text-gray-700 mb-2">显示模式</label>
                <select
                  value={formData.displayMode}
                  onChange={e => handleFieldChange('displayMode', e.target.value)}
                  className="w-full px-3 py-2 border border-gray-300 rounded-md"
                >
                  <option value="GUI">图形界面 (GUI)</option>
                  <option value="Terminal">纯终端 (Console)</option>
                </select>
              </div>
              <div>
                <label className="flex items-center gap-2 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={formData.autoStart}
                    onChange={e => handleFieldChange('autoStart', e.target.checked)}
                    className="w-4 h-4"
                  />
                  <span className="text-sm font-medium text-gray-700">启动时自动运行</span>
                </label>
              </div>
            </div>
          </div>
        );

      case 2:
        return (
          <div className="space-y-6">
            <h3 className="text-lg font-semibold text-gray-900">选择操作系统</h3>
            <div className="grid grid-cols-2 gap-3">
              {osOptions.map(os => (
                <button
                  key={os.id}
                  onClick={() => handleFieldChange('os', os.id)}
                  className={`p-4 rounded-lg border-2 transition-all text-left ${
                    formData.os === os.id
                      ? 'border-blue-600 bg-blue-50'
                      : 'border-gray-200 bg-white hover:border-gray-300'
                  }`}
                >
                  <div className="text-2xl mb-2">{os.icon}</div>
                  <div className="font-medium text-gray-900">{os.name}</div>
                </button>
              ))}
            </div>
          </div>
        );

      case 3:
        return (
          <div className="space-y-6">
            <h3 className="text-lg font-semibold text-gray-900">硬件配置</h3>
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-2">
                CPU 核心数: {formData.cpu}
              </label>
              <input
                type="range"
                min="1"
                max="16"
                value={formData.cpu}
                onChange={e => handleFieldChange('cpu', parseInt(e.target.value))}
                className="w-full"
              />
              <div className="flex justify-between text-xs text-gray-500 mt-1">
                <span>1 核</span>
                <span>16 核</span>
              </div>
            </div>

            <div>
              <label className="block text-sm font-medium text-gray-700 mb-2">
                内存大小: {formData.memory} MB ({(formData.memory / 1024).toFixed(1)} GB)
              </label>
              <input
                type="range"
                min="512"
                max="32768"
                step="512"
                value={formData.memory}
                onChange={e => handleFieldChange('memory', parseInt(e.target.value))}
                className="w-full"
              />
              <div className="flex justify-between text-xs text-gray-500 mt-1">
                <span>512 MB</span>
                <span>32 GB</span>
              </div>
            </div>

            <div>
              <label className="block text-sm font-medium text-gray-700 mb-2">
                磁盘大小: {formData.disk} GB
              </label>
              <input
                type="range"
                min="10"
                max="500"
                step="10"
                value={formData.disk}
                onChange={e => handleFieldChange('disk', parseInt(e.target.value))}
                className="w-full"
              />
              <div className="flex justify-between text-xs text-gray-500 mt-1">
                <span>10 GB</span>
                <span>500 GB</span>
              </div>
            </div>

            <div className="bg-blue-50 border border-blue-200 rounded-lg p-3">
              <p className="text-sm text-blue-900">
                💡 推荐配置: 2-4 CPU, 2-4 GB 内存, 20-50 GB 磁盘
              </p>
            </div>
          </div>
        );

      case 4:
        return (
          <div className="space-y-6">
            <h3 className="text-lg font-semibold text-gray-900">网络设置</h3>
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-2">网络类型</label>
              <div className="space-y-2">
                {[
                  { value: 'bridge', label: '桥接模式', desc: '虚拟机直接连接物理网络' },
                  { value: 'nat', label: 'NAT 模式', desc: '虚拟机通过宿主机上网' },
                  { value: 'internal', label: '内部网络', desc: '虚拟机之间互联' },
                ].map(opt => (
                  <label key={opt.value} className="flex items-start gap-3 cursor-pointer">
                    <input
                      type="radio"
                      name="network"
                      value={opt.value}
                      checked={formData.network === opt.value}
                      onChange={e => handleFieldChange('network', e.target.value)}
                      className="w-4 h-4 mt-1"
                    />
                    <div>
                      <div className="font-medium text-gray-900">{opt.label}</div>
                      <div className="text-sm text-gray-600">{opt.desc}</div>
                    </div>
                  </label>
                ))}
              </div>
            </div>

            <div>
              <label className="flex items-center gap-2 cursor-pointer">
                <input
                  type="checkbox"
                  checked={formData.enableSnapshot}
                  onChange={e => handleFieldChange('enableSnapshot', e.target.checked)}
                  className="w-4 h-4"
                />
                <span className="text-sm font-medium text-gray-700">启用快照功能</span>
              </label>
            </div>
          </div>
        );

      case 5:
        return (
          <div className="space-y-6">
            <h3 className="text-lg font-semibold text-gray-900">确认创建</h3>
            <div className="space-y-4">
              <div className="bg-gray-50 p-4 rounded-lg space-y-2">
                <div className="flex justify-between">
                  <span className="text-gray-600">虚拟机名称</span>
                  <span className="font-medium text-gray-900">{formData.name || '未设置'}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-gray-600">操作系统</span>
                  <span className="font-medium text-gray-900">
                    {osOptions.find(o => o.id === formData.os)?.name}
                  </span>
                </div>
                <div className="flex justify-between">
                  <span className="text-gray-600">CPU / 内存 / 磁盘</span>
                  <span className="font-medium text-gray-900">
                    {formData.cpu} 核 / {(formData.memory / 1024).toFixed(1)} GB / {formData.disk} GB
                  </span>
                </div>
                <div className="flex justify-between">
                  <span className="text-gray-600">网络模式</span>
                  <span className="font-medium text-gray-900">
                    {formData.network === 'bridge' && '桥接'}
                    {formData.network === 'nat' && 'NAT'}
                    {formData.network === 'internal' && '内部'}
                  </span>
                </div>
                <div className="flex justify-between">
                  <span className="text-gray-600">显示模式</span>
                  <span className="font-medium text-gray-900">{formData.displayMode}</span>
                </div>
              </div>

              <div className="bg-amber-50 border border-amber-200 rounded-lg p-3">
                <p className="text-sm text-amber-900">
                  ⚠️ 虚拟机创建后将需要安装操作系统。请准备安装介质 (ISO 文件)
                </p>
              </div>

              {!formData.name && (
                <div className="bg-red-50 border border-red-200 rounded-lg p-3">
                  <p className="text-sm text-red-900">❌ 请返回第一步设置虚拟机名称</p>
                </div>
              )}
            </div>
          </div>
        );

      default:
        return null;
    }
  };

  return (
    <div className="max-w-2xl mx-auto">
      <div className="bg-white rounded-lg border border-gray-200 overflow-hidden">
        {/* 进度条 */}
        <div className="bg-gray-50 border-b border-gray-200 px-6 py-4">
          <div className="flex gap-2">
            {steps.map((step, index) => (
              <React.Fragment key={step.step}>
                <button
                  onClick={() => setCurrentStep(step.step)}
                  className={`flex items-center gap-2 px-4 py-2 rounded-lg transition-colors ${
                    currentStep >= step.step
                      ? 'bg-blue-600 text-white'
                      : 'bg-gray-200 text-gray-600'
                  }`}
                >
                  <span>{step.icon}</span>
                  <span className="text-sm font-medium">{step.name}</span>
                </button>
                {index < steps.length - 1 && (
                  <div
                    className={`flex-1 h-1 ${
                      currentStep > step.step ? 'bg-blue-600' : 'bg-gray-200'
                    }`}
                  ></div>
                )}
              </React.Fragment>
            ))}
          </div>
        </div>

        {/* 内容 */}
        <div className="p-6 min-h-64">{renderStep()}</div>

        {/* 按钮 */}
        <div className="border-t border-gray-200 bg-gray-50 px-6 py-4 flex justify-between">
          <div className="flex gap-2">
            <Button
              variant="outline"
              onClick={handlePrev}
              disabled={currentStep === 1}
            >
              ← 上一步
            </Button>
            <Button
              variant="primary"
              onClick={handleNext}
              disabled={currentStep === steps.length}
            >
              下一步 →
            </Button>
          </div>
          <div className="flex gap-2">
            <Button variant="outline" onClick={onCancel}>
              取消
            </Button>
            {currentStep === steps.length && (
              <Button
                variant="success"
                onClick={handleCreate}
                disabled={!formData.name}
              >
                ✓ 创建虚拟机
              </Button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
};
