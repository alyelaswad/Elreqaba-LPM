const { app, BrowserWindow, ipcMain } = require('electron');
const path = require('path');
const { exec } = require('child_process');
const util = require('util');
const execPromise = util.promisify(exec);
const fs = require('fs');

let mainWindow;
let updateInterval;
let trackedProcesses = new Set();

async function getAppIconPath(processName) {
    if (process.platform !== 'darwin') return null;
    
    try {
        if (!processName.toLowerCase().endsWith('.app') && !processName.includes('.')) {
            return null;
        }

        const appName = processName.endsWith('.app') ? 
            processName : 
            processName.split('.')[0] + '.app';

        const { stdout } = await execPromise(`mdfind "kMDItemKind == 'Application' && kMDItemDisplayName == '${appName}'"`).catch(() => ({ stdout: '' }));
        const appPath = stdout.trim().split('\n')[0];
        
        if (appPath) {
            const iconPath = path.join(appPath, 'Contents', 'Resources', 'AppIcon.icns');
            if (fs.existsSync(iconPath)) {
                return iconPath;
            }
        }
    } catch (error) {
        return null;
    }
    return null;
}

async function getProcessCommand(pid) {
    try {
        const { stdout } = await execPromise(`ps -p ${pid} -o command=`);
        return stdout.trim();
    } catch (error) {
        return null;
    }
}

function createWindow() {
    mainWindow = new BrowserWindow({
        width: 1000,
        height: 700,
        webPreferences: {
            nodeIntegration: true,
            contextIsolation: false
        }
    });

    mainWindow.loadFile('index.html');
}

async function getProcesses() {
    try {
        const { stdout } = await execPromise('ps -ax -o pid,ppid,pcpu,pmem,comm,state,command');
        const processLines = stdout.split('\n')
            .slice(1) 
            .filter(line => line.trim());

        const processPromises = processLines.map(async line => {
            try {
                const parts = line.trim().split(/\s+/);
                const pid = parts[0];
                const ppid = parts[1];
                const cpu = parts[2];
                const mem = parts[3];
                const comm = parts[4];
                const state = parts[5];
                const command = await getProcessCommand(pid).catch(() => parts.slice(6).join(' '));

                let name = '';
                if (command && command.includes('/')) {
                    // If command has a path, get the last part
                    const parts = command.split('/');
                    name = parts[parts.length - 1].split(' ')[0];
                } else {
                    name = comm && comm.includes('/') ? 
                        comm.split('/').pop() : 
                        comm;
                }

                if (!name || name === '' || name === '.' || name === 'ps') {
                    name = comm || 'Unknown';
                }
                
                const stateMap = {
                    'R': 'Running',
                    'S': 'Sleeping',
                    'I': 'Idle',
                    'T': 'Stopped',
                    'U': 'Uninterruptible Sleep',
                    'Z': 'Zombie',
                    'D': 'Uninterruptible Sleep'
                };

                let processState = state && stateMap[state[0]] ? stateMap[state[0]] : state;
                if (state) {
                    if (state.includes('+')) processState += ' (Foreground)';
                    if (state.includes('s')) processState += ' (Session Leader)';
                    if (state.includes('<')) processState += ' (High Priority)';
                    if (state.includes('N')) processState += ' (Low Priority)';
                }

                let iconPath = null;
                try {
                    iconPath = await getAppIconPath(name);
                } catch (error) {
                    iconPath = null;
                }

                return {
                    pid: parseInt(pid),
                    ppid: parseInt(ppid),
                    cpu: parseFloat(cpu) || 0,
                    memory: parseFloat(mem) * 1024 * 1024 || 0, // Convert percentage to bytes
                    name,
                    state: processState || 'Unknown',
                    command: command || '',
                    iconPath
                };
            } catch (error) {
                console.error('Error processing process line:', error);
                return null;
            }
        });

        const processes = (await Promise.all(processPromises))
            .filter(process => process !== null && process.name && process.name.trim() !== '')
            .sort((a, b) => b.cpu - a.cpu)
            .slice(0, 50); // get top 50 processes for better grouping

        let totalMemory = 0;
        let usedMemory = 0;
        let cpuUsage = 0;

        if (process.platform === 'darwin') {
            try {
                const { stdout: memInfo } = await execPromise('vm_stat');
                const pageSize = 4096;
                const memStats = {};
                
                memInfo.split('\n').forEach(line => {
                    const match = line.match(/^(.+):\s+(\d+)/);
                    if (match) {
                        memStats[match[1]] = parseInt(match[2]) * pageSize;
                    }
                });

                totalMemory = memStats['Pages free'] + memStats['Pages active'] + memStats['Pages inactive'] + memStats['Pages wired down'];
                usedMemory = totalMemory - memStats['Pages free'];

                // macOS CPU info
                const { stdout: cpuInfo } = await execPromise("top -l 1 -n 0 | grep 'CPU usage'");
                const cpuMatch = cpuInfo.match(/(\d+\.\d+)% user/);
                cpuUsage = cpuMatch ? parseFloat(cpuMatch[1]) : 0;
            } catch (error) {
                console.error('Error getting system metrics:', error);
            }
        } else {
            try {
                // Linux memory info
                const { stdout: memInfo } = await execPromise('free -b');
                const memLines = memInfo.split('\n');
                const memParts = memLines[1].split(/\s+/);
                totalMemory = parseInt(memParts[1]);
                usedMemory = parseInt(memParts[2]);

                // Linux CPU info
                const { stdout: cpuInfo } = await execPromise("top -bn1 | grep '%Cpu'");
                const cpuParts = cpuInfo.split(/\s+/);
                cpuUsage = parseFloat(cpuParts[1]);
            } catch (error) {
                console.error('Error getting system metrics:', error);
            }
        }

        return {
            processes,
            metrics: {
                totalMemory,
                usedMemory,
                cpuUsage,
                timestamp: new Date().toLocaleTimeString(),
                processData: processes.map(p => ({
                    name: p.name,
                    cpu: p.cpu,
                    memory: p.memory
                }))
            }
        };
    } catch (error) {
        console.error('Error getting processes:', error);
        return { 
            processes: [], 
            metrics: { 
                totalMemory: 0,
                usedMemory: 0,
                cpuUsage: 0, 
                timestamp: new Date().toLocaleTimeString(),
                processData: []
            } 
        };
    }
}

async function getTrackedProcesses() {
    try {
        const trackedProcessesData = [];
        const processesToRemove = new Set();
        
        for (const pid of trackedProcesses) {
            try {
                const { stdout } = await execPromise(`ps -p ${pid} -o pid,ppid,pcpu,pmem,comm,stat,command`);
                const lines = stdout.split('\n').slice(1).filter(line => line.trim());
                if (lines.length > 0) {
                    const [pid, ppid, cpu, mem, comm, stat, ...cmdParts] = lines[0].trim().split(/\s+/);
                    const command = cmdParts.join(' ');
                    let name = command.split('/').pop().split(' ')[0];
                    if (name === '') name = comm;
                    
                    trackedProcessesData.push({
                        pid: parseInt(pid),
                        ppid: parseInt(ppid),
                        cpu: parseFloat(cpu),
                        memory: parseFloat(mem) * 1024 * 1024,
                        name,
                        state: stat,
                        command
                    });
                } else {
                    // process doesn't exist anymore, mark for removal
                    processesToRemove.add(pid);
                }
            } catch (error) {
                //process doesn't exist or error occurred, mark for removal
                processesToRemove.add(pid);
            }
        }
        
        // Remove processes that no longer exist
        for (const pid of processesToRemove) {
            trackedProcesses.delete(pid);
        }
        
        return trackedProcessesData;
    } catch (error) {
        console.error('Error getting tracked processes:', error);
        return [];
    }
}

// drop down menu 
async function handleProcessAction(action, pid) {
    try {
        switch (action) {
            case 'kill':
                try {
                    // First try normal kill
                    await execPromise(`kill -9 ${pid}`);
                } catch (error) {
                    console.log(`Normal kill failed for PID ${pid}, error:`, error);
                    // If normal kill fails, try with sudo
                    try {
                        await execPromise(`sudo -n kill -9 ${pid}`);
                    } catch (sudoError) {
                        console.log(`Sudo kill failed for PID ${pid}, error:`, sudoError);
                        throw new Error(`Cannot terminate process ${pid}. This may be a system process that requires elevated privileges.`);
                    }
                }
                mainWindow.webContents.send('process-action-result', {
                    success: true,
                    action,
                    pid
                });
                break;
            case 'pause':
                await execPromise(`kill -STOP ${pid}`);
                mainWindow.webContents.send('process-action-result', {
                    success: true,
                    action,
                    pid
                });
                break;
            case 'resume':
                await execPromise(`kill -CONT ${pid}`);
                mainWindow.webContents.send('process-action-result', {
                    success: true,
                    action,
                    pid
                });
                break;
            case 'priority':
                // Not implemented yet
                mainWindow.webContents.send('process-action-result', {
                    success: false,
                    error: 'Priority change not implemented yet',
                    action,
                    pid
                });
                break;
        }
    } catch (error) {
        console.error(`Error performing action ${action} on process ${pid}:`, error);
        mainWindow.webContents.send('process-action-result', {
            success: false,
            error: error.message,
            action,
            pid
        });
    }
}

app.whenReady().then(() => {
    createWindow();

    updateInterval = setInterval(async () => {
        if (mainWindow) {
            const processes = await getProcesses();
            const tracked = await getTrackedProcesses();
            mainWindow.webContents.send('process-update', {
                ...processes,
                trackedProcesses: tracked
            });
        }
    }, 2000);

    ipcMain.on('process-action', async (event, { action, pid }) => {
        await handleProcessAction(action, pid);
    });

    ipcMain.on('track-process', (event, pid) => {
        trackedProcesses.add(parseInt(pid));
    });

    ipcMain.on('untrack-process', (event, pid) => {
        trackedProcesses.delete(parseInt(pid));
    });

    ipcMain.on('refresh-processes', async () => {
        const processes = await getProcesses();
        const tracked = await getTrackedProcesses();
        mainWindow.webContents.send('process-update', {
            ...processes,
            trackedProcesses: tracked
        });
    });

    app.on('activate', () => {
        if (BrowserWindow.getAllWindows().length === 0) {
            createWindow();
        }
    });
});

app.on('window-all-closed', () => {
    if (process.platform !== 'darwin') {
        clearInterval(updateInterval);
        app.quit();
    }
}); 