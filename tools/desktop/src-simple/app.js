// ========================================
// VM Manager - 应用程序逻辑
// ========================================

// 应用状态
const AppState = {
    vms: [],
    selectedVmId: null,
    currentView: 'dashboard'
};

// ========================================
// 控制台状态
// ========================================
const ConsoleState = {
    activeVmId: null,
    autoScroll: true,
    consoleInterval: null,
    maxLines: 1000 // Maximum lines to keep in console
};

// ========================================
// 初始化
// ========================================
document.addEventListener('DOMContentLoaded', async () => {
    initializeApp();
    setupEventListeners();
    await loadVMs();
    startPeriodicUpdates();
});

function initializeApp() {
    console.log('VM Manager 初始化中...');
    updateStats();
}

function setupEventListeners() {
    // 导航菜单
    document.querySelectorAll('.nav-item').forEach(item => {
        item.addEventListener('click', () => {
            const view = item.dataset.view;
            switchView(view);
        });
    });

    // 创建虚拟机按钮
    document.getElementById('btnCreateVm').addEventListener('click', openCreateVMModal);
    document.getElementById('quickCreateVm').addEventListener('click', openCreateVMModal);

    // 刷新按钮
    document.getElementById('btnRefresh').addEventListener('click', async () => {
        await loadVMs();
        showNotification('已刷新', 'success');
    });

    // 模态框
    document.getElementById('closeModal').addEventListener('click', closeCreateVMModal);
    document.getElementById('cancelCreate').addEventListener('click', closeCreateVMModal);
    document.getElementById('closeDetailModal').addEventListener('click', closeVMDetailModal);

    // 表单提交
    document.getElementById('createVmForm').addEventListener('submit', handleCreateVM);

    // 搜索和过滤
    document.getElementById('vmSearch').addEventListener('input', filterVMs);
    document.getElementById('vmFilter').addEventListener('change', filterVMs);

    // 快速操作
    document.getElementById('quickStartAll').addEventListener('click', handleStartAll);
    document.getElementById('quickStopAll').addEventListener('click', handleStopAll);

    // 控制台按钮
    document.getElementById('btnClearConsole').addEventListener('click', clearConsole);
    document.getElementById('btnScrollConsole').addEventListener('click', toggleAutoScroll);
    document.getElementById('chkAutoScroll').addEventListener('change', (e) => {
        ConsoleState.autoScroll = e.target.checked;
    });

    // 点击模态框外部关闭
    document.querySelectorAll('.modal').forEach(modal => {
        modal.addEventListener('click', (e) => {
            if (e.target === modal) {
                modal.classList.remove('active');
            }
        });
    });
}

// ========================================
// 视图切换
// ========================================
function switchView(viewName) {
    // 更新导航状态
    document.querySelectorAll('.nav-item').forEach(item => {
        item.classList.toggle('active', item.dataset.view === viewName);
    });

    // 更新视图显示
    document.querySelectorAll('.view').forEach(view => {
        view.classList.toggle('active', view.id === `view-${viewName}`);
    });

    // 更新页面标题
    const titles = {
        'dashboard': { title: '概览', subtitle: '系统状态总览' },
        'vms': { title: '虚拟机', subtitle: '管理您的虚拟机' },
        'monitoring': { title: '监控', subtitle: '性能监控和分析' },
        'settings': { title: '设置', subtitle: '配置和偏好' }
    };

    const titleInfo = titles[viewName];
    document.getElementById('pageTitle').textContent = titleInfo.title;
    document.getElementById('pageSubtitle').textContent = titleInfo.subtitle;

    AppState.currentView = viewName;

    // 如果切换到虚拟机视图，渲染VM列表
    if (viewName === 'vms') {
        renderVMGrid();
    }
}

// ========================================
// VM 管理
// ========================================
async function loadVMs() {
    try {
        if (window.__TAURI__) {
            // 使用 Tauri API
            AppState.vms = await window.__TAURI__.invoke('list_vms');
        } else {
            // 开发模式 - 使用模拟数据
            AppState.vms = getMockVMs();
        }
        updateStats();
        if (AppState.currentView === 'vms') {
            renderVMGrid();
        }
    } catch (error) {
        console.error('加载虚拟机失败:', error);
        showError('无法加载虚拟机列表');
    }
}

function getMockVMs() {
    return [
        {
            id: '1',
            name: 'Ubuntu Server',
            state: 'Running',
            cpu_count: 4,
            memory_mb: 8192,
            disk_gb: 100,
            display_mode: 'Terminal'
        },
        {
            id: '2',
            name: 'Windows 11',
            state: 'Stopped',
            cpu_count: 8,
            memory_mb: 16384,
            disk_gb: 200,
            display_mode: 'GUI'
        },
        {
            id: '3',
            name: 'Debian Testing',
            state: 'Paused',
            cpu_count: 2,
            memory_mb: 4096,
            disk_gb: 50,
            display_mode: 'Terminal'
        }
    ];
}

async function createVM(config) {
    try {
        if (window.__TAURI__) {
            await window.__TAURI__.invoke('create_vm', { config });
        } else {
            // 开发模式 - 模拟创建
            const newVM = {
                id: Date.now().toString(),
                ...config,
                state: 'Stopped'
            };
            AppState.vms.push(newVM);
        }
        await loadVMs();
        showNotification('虚拟机创建成功', 'success');
        addActivity(`创建虚拟机: ${config.name}`);
    } catch (error) {
        console.error('创建虚拟机失败:', error);
        showError('无法创建虚拟机');
    }
}

async function startVM(vmId) {
    try {
        if (window.__TAURI__) {
            await window.__TAURI__.invoke('start_vm', { id: vmId });
        } else {
            // 开发模式
            const vm = AppState.vms.find(v => v.id === vmId);
            if (vm) vm.state = 'Running';
        }
        await loadVMs();
        showNotification('虚拟机已启动', 'success');
        addActivity(`启动虚拟机: ${getVMName(vmId)}`);
    } catch (error) {
        console.error('启动虚拟机失败:', error);
        showError('无法启动虚拟机');
    }
}

async function stopVM(vmId) {
    try {
        if (window.__TAURI__) {
            await window.__TAURI__.invoke('stop_vm', { id: vmId });
        } else {
            const vm = AppState.vms.find(v => v.id === vmId);
            if (vm) vm.state = 'Stopped';
        }
        await loadVMs();
        showNotification('虚拟机已停止', 'success');
        addActivity(`停止虚拟机: ${getVMName(vmId)}`);
    } catch (error) {
        console.error('停止虚拟机失败:', error);
        showError('无法停止虚拟机');
    }
}

async function pauseVM(vmId) {
    try {
        if (window.__TAURI__) {
            await window.__TAURI__.invoke('pause_vm', { id: vmId });
        } else {
            const vm = AppState.vms.find(v => v.id === vmId);
            if (vm) vm.state = 'Paused';
        }
        await loadVMs();
        showNotification('虚拟机已暂停', 'success');
        addActivity(`暂停虚拟机: ${getVMName(vmId)}`);
    } catch (error) {
        console.error('暂停虚拟机失败:', error);
        showError('无法暂停虚拟机');
    }
}

async function deleteVM(vmId) {
    try {
        if (window.__TAURI__) {
            await window.__TAURI__.invoke('delete_vm', { id: vmId });
        } else {
            AppState.vms = AppState.vms.filter(v => v.id !== vmId);
        }
        await loadVMs();
        showNotification('虚拟机已删除', 'success');
        addActivity(`删除虚拟机: ${getVMName(vmId)}`);
    } catch (error) {
        console.error('删除虚拟机失败:', error);
        showError('无法删除虚拟机');
    }
}

// ========================================
// UI 更新
// ========================================
function updateStats() {
    const total = AppState.vms.length;
    const running = AppState.vms.filter(vm => vm.state === 'Running').length;
    const stopped = AppState.vms.filter(vm => vm.state === 'Stopped').length;
    const totalMemory = AppState.vms.reduce((sum, vm) => sum + vm.memory_mb, 0);

    document.getElementById('statTotalVms').textContent = total;
    document.getElementById('statRunningVms').textContent = running;
    document.getElementById('statStoppedVms').textContent = stopped;
    document.getElementById('statTotalMemory').textContent = `${(totalMemory / 1024).toFixed(1)} GB`;
    document.getElementById('vmCount').textContent = running;
}

function renderVMGrid() {
    const grid = document.getElementById('vmGrid');
    const searchTerm = document.getElementById('vmSearch').value.toLowerCase();
    const filter = document.getElementById('vmFilter').value;

    let filteredVMs = AppState.vms;

    // 应用搜索过滤
    if (searchTerm) {
        filteredVMs = filteredVMs.filter(vm =>
            vm.name.toLowerCase().includes(searchTerm)
        );
    }

    // 应用状态过滤
    if (filter !== 'all') {
        filteredVMs = filteredVMs.filter(vm =>
            vm.state.toLowerCase() === filter.toLowerCase()
        );
    }

    if (filteredVMs.length === 0) {
        grid.innerHTML = `
            <div class="empty-state" style="grid-column: 1 / -1;">
                <div class="empty-state-icon">🖥️</div>
                <div class="empty-state-text">没有找到虚拟机</div>
                <div class="empty-state-subtext">创建您的第一个虚拟机开始使用</div>
            </div>
        `;
        return;
    }

    grid.innerHTML = filteredVMs.map(vm => `
        <div class="vm-card" data-vm-id="${vm.id}">
            <div class="vm-card-header">
                <div class="vm-card-icon">💻</div>
                <span class="vm-card-status ${vm.state.toLowerCase()}">${vm.state}</span>
            </div>
            <div class="vm-card-title">${vm.name}</div>
            <div class="vm-card-specs">
                <div>📊 ${vm.cpu_count} 核心</div>
                <div>💾 ${vm.memory_mb} MB</div>
                <div>💿 ${vm.disk_gb} GB</div>
            </div>
            <div class="vm-card-actions">
                ${vm.state === 'Stopped' ?
                    `<button class="vm-card-btn primary" onclick="event.stopPropagation(); startVM('${vm.id}')">▶️ 启动</button>` :
                    vm.state === 'Running' ?
                    `<button class="vm-card-btn" onclick="event.stopPropagation(); pauseVM('${vm.id}')">⏸️ 暂停</button>` :
                    `<button class="vm-card-btn" onclick="event.stopPropagation(); startVM('${vm.id}')">▶️ 继续</button>`
                }
                <button class="vm-card-btn" onclick="event.stopPropagation(); showVMDetail('${vm.id}')">详情</button>
            </div>
        </div>
    `).join('');

    // 添加点击事件显示详情
    grid.querySelectorAll('.vm-card').forEach(card => {
        card.addEventListener('click', () => {
            const vmId = card.dataset.vmId;
            showVMDetail(vmId);
        });
    });
}

function filterVMs() {
    renderVMGrid();
}

// ========================================
// 模态框
// ========================================
function openCreateVMModal() {
    document.getElementById('createVmModal').classList.add('active');
}

function closeCreateVMModal() {
    document.getElementById('createVmModal').classList.remove('active');
    document.getElementById('createVmForm').reset();
}

function showVMDetail(vmId) {
    const vm = AppState.vms.find(v => v.id === vmId);
    if (!vm) return;

    AppState.selectedVmId = vmId;

    // 更新详情内容
    document.getElementById('detailVmName').textContent = vm.name;
    document.getElementById('detailStatus').textContent = vm.state;
    document.getElementById('detailCpu').textContent = `${vm.cpu_count} 核心`;
    document.getElementById('detailMemory').textContent = `${vm.memory_mb} MB`;
    document.getElementById('detailDisk').textContent = `${vm.disk_gb} GB`;

    // 更新指标（模拟数据）
    document.getElementById('metricCpu').textContent = vm.state === 'Running' ? `${(Math.random() * 50 + 10).toFixed(0)}%` : '0%';
    document.getElementById('metricMemory').textContent = vm.state === 'Running' ? `${(vm.memory_mb * (Math.random() * 0.5 + 0.3)).toFixed(0)} MB` : '0 MB';
    document.getElementById('metricUptime').textContent = vm.state === 'Running' ? `${Math.floor(Math.random() * 24)}h` : '0h';

    // 设置按钮事件
    document.getElementById('btnStartVm').onclick = () => { startVM(vmId); };
    document.getElementById('btnPauseVm').onclick = () => { pauseVM(vmId); };
    document.getElementById('btnStopVm').onclick = () => { stopVM(vmId); };
    document.getElementById('btnDeleteVm').onclick = () => {
        if (confirm('确定要删除这个虚拟机吗？')) {
            deleteVM(vmId);
            closeVMDetailModal();
        }
    };

    // 启动控制台流（如果VM正在运行）
    if (vm.state === 'Running') {
        startConsoleStreaming(vmId);
    } else {
        clearConsole();
        appendConsoleLine('虚拟机未运行，无法显示控制台输出', 'warning');
    }

    document.getElementById('vmDetailModal').classList.add('active');
}

function closeVMDetailModal() {
    document.getElementById('vmDetailModal').classList.remove('active');
    stopConsoleStreaming();
    AppState.selectedVmId = null;
}

// ========================================
// 表单处理
// ========================================
async function handleCreateVM(e) {
    e.preventDefault();

    const config = {
        name: document.getElementById('vmName').value,
        cpu_count: parseInt(document.getElementById('cpuCount').value),
        memory_mb: parseInt(document.getElementById('memoryMb').value),
        disk_gb: parseInt(document.getElementById('diskSize').value),
        display_mode: document.getElementById('displayMode').value
    };

    await createVM(config);
    closeCreateVMModal();
}

// ========================================
// 批量操作
// ========================================
async function handleStartAll() {
    const stoppedVMs = AppState.vms.filter(vm => vm.state === 'Stopped');
    if (stoppedVMs.length === 0) {
        showNotification('没有已停止的虚拟机', 'warning');
        return;
    }

    for (const vm of stoppedVMs) {
        await startVM(vm.id);
    }
    showNotification(`已启动 ${stoppedVMs.length} 个虚拟机`, 'success');
}

async function handleStopAll() {
    const runningVMs = AppState.vms.filter(vm => vm.state === 'Running');
    if (runningVMs.length === 0) {
        showNotification('没有运行中的虚拟机', 'warning');
        return;
    }

    if (!confirm(`确定要停止所有 ${runningVMs.length} 个运行中的虚拟机吗？`)) {
        return;
    }

    for (const vm of runningVMs) {
        await stopVM(vm.id);
    }
    showNotification(`已停止 ${runningVMs.length} 个虚拟机`, 'success');
}

// ========================================
// 活动日志
// ========================================
function addActivity(text) {
    const activityList = document.getElementById('activityList');
    const now = new Date();
    const timeStr = '刚刚';

    const activityItem = document.createElement('div');
    activityItem.className = 'activity-item';

    const timeSpan = document.createElement('span');
    timeSpan.className = 'activity-time';
    timeSpan.textContent = timeStr;

    const textSpan = document.createElement('span');
    textSpan.className = 'activity-text';
    textSpan.textContent = text;

    activityItem.appendChild(timeSpan);
    activityItem.appendChild(textSpan);

    activityList.insertBefore(activityItem, activityList.firstChild);

    // 只保留最近 10 条
    while (activityList.children.length > 10) {
        activityList.removeChild(activityList.lastChild);
    }
}

// ========================================
// 通知
// ========================================
function showNotification(message, type = 'info') {
    console.log(`[${type.toUpperCase()}] ${message}`);

    const notification = document.createElement('div');
    notification.style.cssText = `
        position: fixed;
        top: 20px;
        right: 20px;
        padding: 1rem 1.5rem;
        background: ${type === 'success' ? '#10b981' : type === 'error' ? '#ef4444' : '#6366f1'};
        color: white;
        border-radius: 8px;
        box-shadow: 0 10px 15px -3px rgba(0, 0, 0, 0.1);
        z-index: 9999;
        animation: slideIn 0.3s ease;
    `;
    notification.textContent = message;
    document.body.appendChild(notification);

    setTimeout(() => {
        notification.style.animation = 'slideOut 0.3s ease';
        setTimeout(() => notification.remove(), 300);
    }, 3000);
}

function showError(message) {
    showNotification(message, 'error');
}

// ========================================
// 工具函数
// ========================================
function getVMName(vmId) {
    const vm = AppState.vms.find(v => v.id === vmId);
    return vm ? vm.name : vmId;
}

// ========================================
// 控制台功能
// ========================================

// 清空控制台
function clearConsole() {
    const consoleOutput = document.getElementById('consoleOutput');
    while (consoleOutput.firstChild) {
        consoleOutput.removeChild(consoleOutput.firstChild);
    }
}

// 切换自动滚动
function toggleAutoScroll() {
    ConsoleState.autoScroll = !ConsoleState.autoScroll;
    document.getElementById('chkAutoScroll').checked = ConsoleState.autoScroll;
}

// 格式化时间戳
function formatTimestamp() {
    const now = new Date();
    const hours = String(now.getHours()).padStart(2, '0');
    const minutes = String(now.getMinutes()).padStart(2, '0');
    const seconds = String(now.getSeconds()).padStart(2, '0');
    return `${hours}:${minutes}:${seconds}`;
}

// 添加控制台行（XSS安全 - 使用textContent）
function appendConsoleLine(message, type = 'user') {
    const consoleOutput = document.getElementById('consoleOutput');

    // 创建控制台行元素
    const line = document.createElement('div');
    line.className = `console-line console-${type}`;

    // 添加时间戳
    const timestamp = document.createElement('span');
    timestamp.className = 'console-timestamp';
    timestamp.textContent = formatTimestamp();
    line.appendChild(timestamp);

    // 添加消息（使用textContent防止XSS）
    const messageSpan = document.createElement('span');
    messageSpan.textContent = message;
    line.appendChild(messageSpan);

    consoleOutput.appendChild(line);

    // 限制最大行数
    while (consoleOutput.children.length > ConsoleState.maxLines) {
        consoleOutput.removeChild(consoleOutput.firstChild);
    }

    // 自动滚动
    if (ConsoleState.autoScroll) {
        consoleOutput.scrollTop = consoleOutput.scrollHeight;
    }
}

// 启动控制台流
async function startConsoleStreaming(vmId) {
    stopConsoleStreaming();
    ConsoleState.activeVmId = vmId;

    clearConsole();
    appendConsoleLine('正在连接到虚拟机控制台...', 'info');

    if (window.__TAURI__) {
        try {
            const vm = AppState.vms.find(v => v.id === vmId);
            if (vm && vm.state === 'Running') {
                appendConsoleLine(`已连接到虚拟机: ${vm.name}`, 'success');
                appendConsoleLine('等待控制台输出...', 'info');

                // 启动轮询（需要后端实现get_console_output命令）
                ConsoleState.consoleInterval = setInterval(async () => {
                    await pollConsoleOutput(vmId);
                }, 500);
            } else {
                appendConsoleLine('虚拟机未运行，无法显示控制台输出', 'warning');
            }
        } catch (error) {
            console.error('启动控制台流失败:', error);
            appendConsoleLine('无法连接到控制台: ' + error.message, 'error');
        }
    } else {
        // 开发模式模拟数据
        appendConsoleLine('开发模式: 使用模拟数据', 'info');
        setTimeout(() => {
            appendConsoleLine('VM Manager v0.1.0 启动中...', 'boot');
        }, 300);
        setTimeout(() => {
            appendConsoleLine('检测到 CPU: x86_64 (4 cores)', 'kernel');
            appendConsoleLine('检测到内存: 2048 MB', 'kernel');
        }, 600);
        setTimeout(() => {
            appendConsoleLine('初始化 VirtIO 设备...', 'boot');
            appendConsoleLine('  - VirtIO block device: /dev/vda (100 GB)', 'info');
            appendConsoleLine('  - VirtIO network device: eth0', 'info');
        }, 900);
        setTimeout(() => {
            appendConsoleLine('内核启动完成', 'success');
            appendConsoleLine('正在启动 init 进程...', 'boot');
        }, 1200);
    }
}

// 轮询控制台输出
async function pollConsoleOutput(vmId) {
    if (!window.__TAURI__) return;

    try {
        // TODO: 需要在后端实现get_console_output命令
        // const output = await window.__TAURI__.invoke('get_console_output', { id: vmId });
        // if (output && output.length > 0) {
        //     output.forEach(line => {
        //         appendConsoleLine(line.text, line.type);
        //     });
        // }
    } catch (error) {
        console.error('获取控制台输出失败:', error);
    }
}

// 停止控制台流
function stopConsoleStreaming() {
    if (ConsoleState.consoleInterval) {
        clearInterval(ConsoleState.consoleInterval);
        ConsoleState.consoleInterval = null;
    }
    ConsoleState.activeVmId = null;
}

// ========================================
// 定期更新
// ========================================
function startPeriodicUpdates() {
    // 每 5 秒更新一次数据
    setInterval(async () => {
        await loadVMs();
    }, 5000);

    // 每 1 秒更新一次性能指标 (Session 13 - XSS安全实现)
    setInterval(async () => {
        await updateMetrics();
    }, 1000);
}

// ========================================
// 实时性能指标更新 (Session 13 - XSS安全实现)
// ========================================

// 缓存当前指标数据
const MetricsCache = {
    vmMetrics: new Map(), // vmId -> VmMetrics
    systemMetrics: null,  // SystemMetrics
    lastUpdate: 0
};

// 主更新函数
async function updateMetrics() {
    try {
        // 并行获取所有指标
        const [allMetrics, systemMetrics] = await Promise.all([
            getAllMetrics(),
            getSystemMetrics()
        ]);

        // 更新缓存
        MetricsCache.vmMetrics.clear();
        allMetrics.forEach(metric => {
            MetricsCache.vmMetrics.set(metric.id, metric);
        });
        MetricsCache.systemMetrics = systemMetrics;
        MetricsCache.lastUpdate = Date.now();

        // 根据当前视图更新UI
        updateDashboardMetrics(systemMetrics);

        if (AppState.currentView === 'monitoring') {
            updateMonitoringCharts(allMetrics);
        }

        // 如果有选中的VM，更新详情页指标
        if (AppState.selectedVmId) {
            updateVMDetailMetrics(AppState.selectedVmId);
        }

    } catch (error) {
        console.error('更新性能指标失败:', error);
    }
}

// 获取所有VM指标
async function getAllMetrics() {
    if (window.__TAURI__) {
        try {
            return await window.__TAURI__.invoke('get_all_metrics');
        } catch (error) {
            console.error('获取VM指标失败:', error);
            return [];
        }
    } else {
        // 开发模式 - 模拟数据
        return AppState.vms.map(vm => ({
            id: vm.id,
            cpu_usage: vm.state === 'Running' ? Math.random() * 50 + 10 : 0,
            memory_usage_mb: vm.state === 'Running' ? Math.floor(vm.memory_mb * (Math.random() * 0.5 + 0.3)) : 0,
            disk_io_read_mb_s: vm.state === 'Running' ? Math.random() * 10 : 0,
            disk_io_write_mb_s: vm.state === 'Running' ? Math.random() * 5 : 0,
            network_rx_mb_s: vm.state === 'Running' ? Math.random() * 2 : 0,
            network_tx_mb_s: vm.state === 'Running' ? Math.random() * 1 : 0,
            uptime_secs: vm.state === 'Running' ? Math.floor(Math.random() * 86400) : 0
        }));
    }
}

// 获取系统指标
async function getSystemMetrics() {
    if (window.__TAURI__) {
        try {
            return await window.__TAURI__.invoke('get_system_metrics');
        } catch (error) {
            console.error('获取系统指标失败:', error);
            return null;
        }
    } else {
        // 开发模式 - 聚合模拟数据
        const runningVMs = AppState.vms.filter(vm => vm.state === 'Running');
        const totalCPU = runningVMs.reduce((sum, vm) => sum + (Math.random() * 50 + 10), 0);
        const totalMemory = runningVMs.reduce((sum, vm) => sum + vm.memory_mb, 0);

        return {
            total_vms: AppState.vms.length,
            running_vms: runningVMs.length,
            total_cpu_usage: totalCPU,
            total_memory_mb: totalMemory,
            used_memory_mb: totalMemory,
            total_disk_io_mb_s: runningVMs.reduce((sum, vm) => sum + Math.random() * 10, 0),
            total_network_mb_s: runningVMs.reduce((sum, vm) => sum + Math.random() * 2, 0)
        };
    }
}

// 更新仪表板统计卡片 (XSS安全 - 使用textContent)
function updateDashboardMetrics(systemMetrics) {
    if (!systemMetrics) return;

    // 总VM数
    const statTotalVms = document.getElementById('statTotalVms');
    if (statTotalVms) {
        statTotalVms.textContent = systemMetrics.total_vms;
    }

    // 运行中VM数
    const statRunningVms = document.getElementById('statRunningVms');
    if (statRunningVms) {
        statRunningVms.textContent = systemMetrics.running_vms;
    }

    // 总内存
    const statTotalMemory = document.getElementById('statTotalMemory');
    if (statTotalMemory) {
        const memoryGB = (systemMetrics.total_memory_mb / 1024).toFixed(1);
        statTotalMemory.textContent = `${memoryGB} GB`;
    }

    // CPU使用率
    const statCpuUsage = document.getElementById('vmCount');
    if (statCpuUsage) {
        statCpuUsage.textContent = systemMetrics.running_vms;
    }
}

// 更新监控图表 (XSS安全 - 使用createElement)
function updateMonitoringCharts(allMetrics) {
    updateCPUChart(allMetrics);
    updateMemoryChart(allMetrics);
}

// CPU使用率图表 (XSS安全实现)
function updateCPUChart(allMetrics) {
    const container = document.getElementById('cpuChartContainer');
    if (!container) return;

    // 清空容器
    while (container.firstChild) {
        container.removeChild(container.firstChild);
    }

    // 创建图表标题
    const title = document.createElement('div');
    title.className = 'chart-title';
    title.textContent = 'CPU 使用率';
    container.appendChild(title);

    // 为每个VM创建指标行
    allMetrics.forEach(vm => {
        const vmRow = createVMMetricRow(vm.id, vm.cpu_usage, '%', [
            { threshold: 80, color: '#ef4444' },  // 红色 - 高负载
            { threshold: 50, color: '#f59e0b' },  // 橙色 - 中等
            { threshold: 0, color: '#10b981' }    // 绿色 - 正常
        ]);
        container.appendChild(vmRow);
    });
}

// 内存使用率图表 (XSS安全实现)
function updateMemoryChart(allMetrics) {
    const container = document.getElementById('memoryChartContainer');
    if (!container) return;

    // 清空容器
    while (container.firstChild) {
        container.removeChild(container.firstChild);
    }

    // 创建图表标题
    const title = document.createElement('div');
    title.className = 'chart-title';
    title.textContent = '内存使用';
    container.appendChild(title);

    // 为每个VM创建指标行
    allMetrics.forEach(vm => {
        const memoryGB = (vm.memory_usage_mb / 1024).toFixed(1);
        const vmRow = createVMMetricRow(vm.id, vm.memory_usage_mb, ' MB', [
            { threshold: 80 * 1024, color: '#ef4444' },  // 80GB以上 - 红色
            { threshold: 50 * 1024, color: '#f59e0b' },  // 50GB以上 - 橙色
            { threshold: 0, color: '#10b981' }          // 其他 - 绿色
        ], memoryGB);
        container.appendChild(vmRow);
    });
}

// 创建VM指标行 (XSS安全辅助函数)
function createVMMetricRow(vmId, value, unit, thresholds, displayValue = null) {
    const row = document.createElement('div');
    row.className = 'vm-metric-row';

    // VM名称
    const nameDiv = document.createElement('div');
    nameDiv.className = 'vm-name';
    nameDiv.textContent = vmId;
    row.appendChild(nameDiv);

    // 指标条容器
    const barContainer = document.createElement('div');
    barContainer.className = 'metric-bar-container';

    // 指标条背景
    const barBg = document.createElement('div');
    barBg.className = 'metric-bar-bg';

    // 指标条填充
    const barFill = document.createElement('div');
    barFill.className = 'metric-fill';

    // 根据阈值确定颜色
    const color = thresholds.find(t => value >= t.threshold)?.color || thresholds[thresholds.length - 1].color;
    barFill.style.backgroundColor = color;

    // 计算宽度百分比
    const maxValue = Math.max(...thresholds.map(t => t.threshold));
    const widthPercent = Math.min((value / maxValue) * 100, 100);
    barFill.style.width = `${widthPercent}%`;

    barBg.appendChild(barFill);
    barContainer.appendChild(barBg);
    row.appendChild(barContainer);

    // 指标值文本
    const valueText = document.createElement('div');
    valueText.className = 'metric-value';
    valueText.textContent = displayValue !== null ? `${displayValue}${unit}` : `${value.toFixed(1)}${unit}`;
    row.appendChild(valueText);

    return row;
}

// 更新VM详情页指标 (XSS安全实现)
function updateVMDetailMetrics(vmId) {
    const metrics = MetricsCache.vmMetrics.get(vmId);
    if (!metrics) return;

    // CPU使用率
    const metricCpu = document.getElementById('metricCpu');
    if (metricCpu) {
        metricCpu.textContent = `${metrics.cpu_usage.toFixed(0)}%`;
    }

    // 内存使用
    const metricMemory = document.getElementById('metricMemory');
    if (metricMemory) {
        metricMemory.textContent = `${metrics.memory_usage_mb} MB`;
    }

    // 运行时间
    const metricUptime = document.getElementById('metricUptime');
    if (metricUptime) {
        const hours = Math.floor(metrics.uptime_secs / 3600);
        const minutes = Math.floor((metrics.uptime_secs % 3600) / 60);
        metricUptime.textContent = hours > 0 ? `${hours}h ${minutes}m` : `${minutes}m`;
    }

    // 磁盘I/O (如果元素存在)
    const metricDisk = document.getElementById('metricDisk');
    if (metricDisk) {
        const diskTotal = (metrics.disk_io_read_mb_s + metrics.disk_io_write_mb_s).toFixed(2);
        metricDisk.textContent = `${diskTotal} MB/s`;
    }

    // 网络I/O (如果元素存在)
    const metricNetwork = document.getElementById('metricNetwork');
    if (metricNetwork) {
        const networkTotal = (metrics.network_rx_mb_s + metrics.network_tx_mb_s).toFixed(2);
        metricNetwork.textContent = `${networkTotal} MB/s`;
    }
}

// ========================================
// 动画
// ========================================
const style = document.createElement('style');
style.textContent = `
    @keyframes slideIn {
        from {
            transform: translateX(400px);
            opacity: 0;
        }
        to {
            transform: translateX(0);
            opacity: 1;
        }
    }

    @keyframes slideOut {
        from {
            transform: translateX(0);
            opacity: 1;
        }
        to {
            transform: translateX(400px);
            opacity: 0;
        }
    }
`;
document.head.appendChild(style);

// ========================================
// 导出到全局
// ========================================
window.startVM = startVM;
window.stopVM = stopVM;
window.pauseVM = pauseVM;
window.showVMDetail = showVMDetail;
window.deleteVM = deleteVM;
