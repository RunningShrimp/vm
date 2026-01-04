// ========================================
// VM Manager - Atomic Design JavaScript
// ========================================
// Version: 1.0.0
// Architecture: Component-based with Atomic Design
// ========================================

// ========================================
// Application State
// ========================================

const AppState = {
    vms: [],
    selectedVmId: null,
    currentView: 'dashboard',
    filters: {
        search: '',
        status: 'all'
    }
};

// ========================================
// Mock Data Service
// ========================================

function getMockVMs() {
    return [
        {
            id: 'vm-001',
            name: 'Ubuntu Server 22.04',
            state: 'Running',
            cpu_count: 4,
            memory_mb: 8192,
            disk_gb: 100,
            display_mode: 'GUI',
            cpu_usage: 45,
            memory_usage: 60,
            uptime: '2h 15m'
        },
        {
            id: 'vm-002',
            name: 'Windows 11 Pro',
            state: 'Stopped',
            cpu_count: 8,
            memory_mb: 16384,
            disk_gb: 200,
            display_mode: 'GUI',
            cpu_usage: 0,
            memory_usage: 0,
            uptime: '0h 0m'
        },
        {
            id: 'vm-003',
            name: 'Debian 12',
            state: 'Stopped',
            cpu_count: 2,
            memory_mb: 4096,
            disk_gb: 50,
            display_mode: 'Terminal',
            cpu_usage: 0,
            memory_usage: 0,
            uptime: '0h 0m'
        }
    ];
}

// ========================================
// VM Service
// ========================================

const VMService = {
    async listVMs() {
        if (window.__TAURI__) {
            // Use Tauri backend
            return await window.__TAURI__.invoke('list_vms');
        } else {
            // Use mock data
            await delay(300);
            return getMockVMs();
        }
    },

    async createVM(config) {
        if (window.__TAURI__) {
            return await window.__TAURI__.invoke('create_vm', { config });
        } else {
            await delay(500);
            return {
                id: `vm-${Date.now()}`,
                ...config,
                state: 'Stopped',
                cpu_usage: 0,
                memory_usage: 0,
                uptime: '0h 0m'
            };
        }
    },

    async startVM(vmId) {
        if (window.__TAURI__) {
            return await window.__TAURI__.invoke('start_vm', { vmId });
        } else {
            await delay(1000);
            return { success: true };
        }
    },

    async stopVM(vmId) {
        if (window.__TAURI__) {
            return await window.__TAURI__.invoke('stop_vm', { vmId });
        } else {
            await delay(800);
            return { success: true };
        }
    },

    async pauseVM(vmId) {
        if (window.__TAURI__) {
            return await window.__TAURI__.invoke('pause_vm', { vmId });
        } else {
            await delay(500);
            return { success: true };
        }
    },

    async deleteVM(vmId) {
        if (window.__TAURI__) {
            return await window.__TAURI__.invoke('delete_vm', { vmId });
        } else {
            await delay(600);
            return { success: true };
        }
    }
};

// ========================================
// Utility Functions
// ========================================

function delay(ms) {
    return new Promise(resolve => setTimeout(resolve, ms));
}

function formatMemory(mb) {
    if (mb >= 1024) {
        return `${(mb / 1024).toFixed(1)} GB`;
    }
    return `${mb} MB`;
}

function getStatusBadgeClass(state) {
    const statusMap = {
        'Running': 'atom-status--running',
        'Stopped': 'atom-status--stopped',
        'Paused': 'atom-status--paused'
    };
    return statusMap[state] || 'atom-status--stopped';
}

function getStatusText(state) {
    const statusMap = {
        'Running': '运行中',
        'Stopped': '已停止',
        'Paused': '已暂停'
    };
    return statusMap[state] || state;
}

// ========================================
// Component: VM Card
// ========================================

function createVMCard(vm) {
    const card = document.createElement('div');
    card.className = 'org-vm-card';
    card.dataset.vmId = vm.id;

    const statusClass = getStatusBadgeClass(vm.state);
    const statusText = getStatusText(vm.state);

    card.innerHTML = `
        <div class="org-vm-card__header">
            <span class="org-vm-card__icon">💽</span>
            <span class="atom-status ${statusClass}">${statusText}</span>
        </div>
        <h3 class="org-vm-card__title">${vm.name}</h3>
        <div class="org-vm-card__config">
            <div class="org-vm-card__config-item">📊 ${vm.cpu_count} 核心</div>
            <div class="org-vm-card__config-item">💾 ${formatMemory(vm.memory_mb)}</div>
            <div class="org-vm-card__config-item">💿 ${vm.disk_gb} GB</div>
        </div>
        <div class="org-vm-card__actions">
            <button class="atom-btn atom-btn--primary" data-action="details">详情</button>
            ${vm.state === 'Running'
                ? '<button class="atom-btn atom-btn--warning" data-action="pause">暂停</button>'
                : '<button class="atom-btn atom-btn--success" data-action="start">启动</button>'
            }
        </div>
    `;

    return card;
}

// ========================================
// Component: Notification
// ========================================

function showNotification(message, type = 'info') {
    const container = document.getElementById('notification-container');
    const notification = document.createElement('div');
    notification.className = `org-notification org-notification--${type}`;

    const icons = {
        success: '✅',
        warning: '⚠️',
        danger: '❌',
        info: 'ℹ️'
    };

    notification.innerHTML = `
        <div class="org-notification__icon">${icons[type] || icons.info}</div>
        <div class="org-notification__content">
            <p class="org-notification__title">${type.charAt(0).toUpperCase() + type.slice(1)}</p>
            <p class="org-notification__message">${message}</p>
        </div>
        <button class="org-notification__close" aria-label="Close">×</button>
    `;

    container.appendChild(notification);

    // Auto remove after 3 seconds
    setTimeout(() => {
        notification.style.animation = 'slideIn 0.3s ease reverse';
        setTimeout(() => notification.remove(), 300);
    }, 3000);

    // Close button handler
    notification.querySelector('.org-notification__close').addEventListener('click', () => {
        notification.remove();
    });
}

// ========================================
// Component: Modal
// ========================================

const Modal = {
    open(modalId) {
        const modal = document.getElementById(modalId);
        if (modal) {
            modal.classList.add('org-modal--open');
            document.body.style.overflow = 'hidden';
        }
    },

    close(modalId) {
        const modal = document.getElementById(modalId);
        if (modal) {
            modal.classList.remove('org-modal--open');
            document.body.style.overflow = '';
        }
    },

    closeAll() {
        document.querySelectorAll('.org-modal--open').forEach(modal => {
            modal.classList.remove('org-modal--open');
        });
        document.body.style.overflow = '';
    }
};

// ========================================
// View Manager
// ========================================

function switchView(viewName) {
    // Hide all views
    document.querySelectorAll('[id^="view-"]').forEach(view => {
        view.style.display = 'none';
    });

    // Show target view
    const targetView = document.getElementById(`view-${viewName}`);
    if (targetView) {
        targetView.style.display = '';
        targetView.classList.add('fade-in');
    }

    // Update navigation
    document.querySelectorAll('.org-sidebar__nav-item').forEach(item => {
        item.classList.remove('org-sidebar__nav-item--active');
        if (item.dataset.view === viewName) {
            item.classList.add('org-sidebar__nav-item--active');
        }
    });

    // Update state
    AppState.currentView = viewName;
}

// ========================================
// VM Grid Renderer
// ========================================

async function loadVMs() {
    try {
        const vms = await VMService.listVMs();
        AppState.vms = vms;
        renderVMGrid();
        updateStats();
    } catch (error) {
        showNotification('加载虚拟机列表失败', 'danger');
        console.error('Failed to load VMs:', error);
    }
}

function renderVMGrid() {
    const grid = document.getElementById('vm-grid');
    if (!grid) return;

    // Filter VMs
    let filteredVMs = AppState.vms;

    if (AppState.filters.search) {
        const search = AppState.filters.search.toLowerCase();
        filteredVMs = filteredVMs.filter(vm =>
            vm.name.toLowerCase().includes(search)
        );
    }

    if (AppState.filters.status !== 'all') {
        filteredVMs = filteredVMs.filter(vm =>
            vm.state.toLowerCase() === AppState.filters.status.toLowerCase()
        );
    }

    // Clear grid
    grid.innerHTML = '';

    // Render VM cards
    if (filteredVMs.length === 0) {
        grid.innerHTML = `
            <div class="org-empty-state">
                <div class="org-empty-state__icon">💽</div>
                <h3 class="org-empty-state__title">没有找到虚拟机</h3>
                <p class="org-empty-state__description">
                    ${AppState.filters.search || AppState.filters.status !== 'all'
                        ? '尝试调整搜索条件或过滤器'
                        : '创建您的第一个虚拟机开始使用'}
                </p>
                <div class="org-empty-state__actions">
                    <button class="atom-btn atom-btn--primary" onclick="document.getElementById('btn-create-vm').click()">
                        ➕ 创建虚拟机
                    </button>
                </div>
            </div>
        `;
    } else {
        filteredVMs.forEach(vm => {
            const card = createVMCard(vm);
            grid.appendChild(card);
        });
    }
}

function updateStats() {
    const totalVMs = AppState.vms.length;
    const runningVMs = AppState.vms.filter(vm => vm.state === 'Running').length;
    const stoppedVMs = AppState.vms.filter(vm => vm.state === 'Stopped').length;
    const totalMemory = AppState.vms.reduce((sum, vm) => sum + vm.memory_mb, 0);

    // Update stats in dashboard (if exists)
    const statsElements = document.querySelectorAll('.mol-stat-card__value');
    if (statsElements.length >= 4) {
        statsElements[0].textContent = totalVMs;
        statsElements[1].textContent = runningVMs;
        statsElements[2].textContent = stoppedVMs;
        statsElements[3].textContent = formatMemory(totalMemory);
    }

    // Update VM badge in sidebar
    const badge = document.querySelector('.org-sidebar__nav-item__badge');
    if (badge) {
        badge.textContent = runningVMs;
    }
}

// ========================================
// VM Detail Modal
// ========================================

function showVMDetail(vmId) {
    const vm = AppState.vms.find(v => v.id === vmId);
    if (!vm) return;

    // Update modal content
    document.getElementById('detail-vm-name').textContent = vm.name;
    document.getElementById('detail-vm-id').textContent = `ID: ${vm.id}`;
    document.getElementById('detail-vm-state').textContent = getStatusText(vm.state);
    document.getElementById('detail-vm-cpu').textContent = `${vm.cpu_count} 核心`;
    document.getElementById('detail-vm-memory').textContent = formatMemory(vm.memory_mb);
    document.getElementById('detail-vm-disk').textContent = `${vm.disk_gb} GB`;
    document.getElementById('detail-vm-cpu-usage').textContent = `${vm.cpu_usage}%`;
    document.getElementById('detail-vm-memory-usage').textContent = `${(vm.memory_usage * vm.memory_mb / 100).toFixed(1)} MB`;

    // Update status badge
    const badgeContainer = document.getElementById('detail-vm-status-badge');
    badgeContainer.innerHTML = `<span class="atom-status ${getStatusBadgeClass(vm.state)}">${getStatusText(vm.state)}</span>`;

    // Show modal
    Modal.open('modal-vm-detail');
}

// ========================================
// Event Handlers
// ========================================

function initializeEventHandlers() {
    // Navigation
    document.querySelectorAll('.org-sidebar__nav-item').forEach(item => {
        item.addEventListener('click', (e) => {
            e.preventDefault();
            const viewName = item.dataset.view;
            if (viewName) {
                switchView(viewName);
            }
        });
    });

    // Mobile sidebar toggle
    const toggleBtn = document.querySelector('.org-topbar__toggle');
    if (toggleBtn) {
        toggleBtn.addEventListener('click', () => {
            const sidebar = document.querySelector('.org-app-layout__sidebar');
            sidebar.classList.toggle('org-app-layout__sidebar--open');
        });
    }

    // Create VM buttons
    document.querySelectorAll('[id="btn-create-vm"], [id="btn-create-vm-2"]').forEach(btn => {
        btn.addEventListener('click', () => Modal.open('modal-create-vm'));
    });

    // VM grid actions (event delegation)
    const vmGrid = document.getElementById('vm-grid');
    if (vmGrid) {
        vmGrid.addEventListener('click', async (e) => {
            const card = e.target.closest('.org-vm-card');
            if (!card) return;

            const vmId = card.dataset.vmId;
            const action = e.target.dataset.action;

            if (action === 'details') {
                showVMDetail(vmId);
            } else if (action === 'start') {
                try {
                    await VMService.startVM(vmId);
                    showNotification('虚拟机已启动', 'success');
                    await loadVMs();
                } catch (error) {
                    showNotification('启动虚拟机失败', 'danger');
                }
            } else if (action === 'pause') {
                try {
                    await VMService.pauseVM(vmId);
                    showNotification('虚拟机已暂停', 'success');
                    await loadVMs();
                } catch (error) {
                    showNotification('暂停虚拟机失败', 'danger');
                }
            }
        });
    }

    // Search and filter
    const searchInput = document.getElementById('vm-search');
    const statusFilter = document.getElementById('vm-status-filter');

    if (searchInput) {
        searchInput.addEventListener('input', (e) => {
            AppState.filters.search = e.target.value;
            renderVMGrid();
        });
    }

    if (statusFilter) {
        statusFilter.addEventListener('change', (e) => {
            AppState.filters.status = e.target.value;
            renderVMGrid();
        });
    }

    // Modal close buttons
    document.querySelectorAll('.org-modal__close').forEach(btn => {
        btn.addEventListener('click', () => Modal.closeAll());
    });

    // Modal cancel buttons
    document.querySelectorAll('[data-action="cancel"]').forEach(btn => {
        btn.addEventListener('click', () => Modal.closeAll());
    });

    // Click outside modal to close
    document.querySelectorAll('.org-modal').forEach(modal => {
        modal.addEventListener('click', (e) => {
            if (e.target === modal) {
                Modal.close(modal.id);
            }
        });
    });

    // Create VM form
    const createVMForm = document.getElementById('form-create-vm');
    const createVMConfirm = document.querySelector('[data-action="confirm"]');

    if (createVMConfirm) {
        createVMConfirm.addEventListener('click', async () => {
            if (!createVMForm.checkValidity()) {
                createVMForm.reportValidity();
                return;
            }

            const formData = new FormData(createVMForm);
            const config = {
                name: formData.get('name'),
                cpu_count: parseInt(formData.get('cpu')),
                memory_mb: parseInt(formData.get('memory')),
                disk_gb: parseInt(formData.get('disk')),
                display_mode: formData.get('display')
            };

            try {
                await VMService.createVM(config);
                showNotification(`虚拟机 "${config.name}" 创建成功`, 'success');
                Modal.closeAll();
                createVMForm.reset();
                await loadVMs();
            } catch (error) {
                showNotification('创建虚拟机失败', 'danger');
            }
        });
    }

    // Start/Stop all buttons
    const startAllBtn = document.getElementById('btn-start-all');
    const stopAllBtn = document.getElementById('btn-stop-all');

    if (startAllBtn) {
        startAllBtn.addEventListener('click', async () => {
            const stoppedVMs = AppState.vms.filter(vm => vm.state !== 'Running');
            if (stoppedVMs.length === 0) {
                showNotification('没有可启动的虚拟机', 'warning');
                return;
            }

            if (!confirm(`确定要启动所有 ${stoppedVMs.length} 个虚拟机吗？`)) {
                return;
            }

            try {
                for (const vm of stoppedVMs) {
                    await VMService.startVM(vm.id);
                }
                showNotification(`已启动 ${stoppedVMs.length} 个虚拟机`, 'success');
                await loadVMs();
            } catch (error) {
                showNotification('批量启动失败', 'danger');
            }
        });
    }

    if (stopAllBtn) {
        stopAllBtn.addEventListener('click', async () => {
            const runningVMs = AppState.vms.filter(vm => vm.state === 'Running');
            if (runningVMs.length === 0) {
                showNotification('没有运行中的虚拟机', 'warning');
                return;
            }

            if (!confirm(`确定要停止所有 ${runningVMs.length} 个虚拟机吗？`)) {
                return;
            }

            try {
                for (const vm of runningVMs) {
                    await VMService.stopVM(vm.id);
                }
                showNotification(`已停止 ${runningVMs.length} 个虚拟机`, 'success');
                await loadVMs();
            } catch (error) {
                showNotification('批量停止失败', 'danger');
            }
        });
    }

    // Keyboard shortcuts
    document.addEventListener('keydown', (e) => {
        if (e.key === 'Escape') {
            Modal.closeAll();
        }
    });
}

// ========================================
// Application Initialization
// ========================================

async function initApp() {
    console.log('🚀 VM Manager initializing...');

    // Initialize event handlers
    initializeEventHandlers();

    // Load VMs
    await loadVMs();

    // Set initial view
    switchView('dashboard');

    console.log('✅ VM Manager initialized');
}

// ========================================
// Start Application
// ========================================

if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', initApp);
} else {
    initApp();
}

// ========================================
// Auto-refresh (every 5 seconds)
// ========================================

setInterval(() => {
    if (AppState.currentView === 'dashboard' || AppState.currentView === 'vms') {
        loadVMs();
    }
}, 5000);
