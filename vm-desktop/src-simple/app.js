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
    document.getElementById('btnStartVm').onclick = () => { startVM(vmId); closeVMDetailModal(); };
    document.getElementById('btnPauseVm').onclick = () => { pauseVM(vmId); closeVMDetailModal(); };
    document.getElementById('btnStopVm').onclick = () => { stopVM(vmId); closeVMDetailModal(); };
    document.getElementById('btnDeleteVm').onclick = () => {
        if (confirm('确定要删除这个虚拟机吗？')) {
            deleteVM(vmId);
            closeVMDetailModal();
        }
    };

    document.getElementById('vmDetailModal').classList.add('active');
}

function closeVMDetailModal() {
    document.getElementById('vmDetailModal').classList.remove('active');
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
    activityItem.innerHTML = `
        <span class="activity-time">${timeStr}</span>
        <span class="activity-text">${text}</span>
    `;

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
    // 简单的通知实现
    console.log(`[${type.toUpperCase()}] ${message}`);

    // 可以在这里添加更复杂的通知UI
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
// 定期更新
// ========================================
function startPeriodicUpdates() {
    // 每 5 秒更新一次数据
    setInterval(async () => {
        await loadVMs();
    }, 5000);
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
