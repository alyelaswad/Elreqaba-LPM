const { app, BrowserWindow, ipcMain } = require('electron');
const path = require('path');
const { exec } = require('child_process');
const util = require('util');
const execPromise = util.promisify(exec);

let mainWindow;
let updateInterval;
let trackedProcesses = new Set();

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
        const { stdout } = await execPromise('ps -eo pid,pcpu,pmem,comm,stat,command');
        const processes = stdout.split('\n')
            .slice(1) 
            .filter(line => line.trim())
            .map(line => {
                const [pid, cpu, mem, comm, stat, ...cmdParts] = line.trim().split(/\s+/);
                const command = cmdParts.join(' ');
                let name = command.split('/').pop().split(' ')[0];
                if (name === '') name = comm;
                const stateMap = {
                    'R': 'Running',
                    'S': 'Sleeping',
                    'D': 'Uninterruptible Sleep',
                    'Z': 'Zombie',
                    'T': 'Stopped',
                    't': 'Tracing Stop',
                    'X': 'Dead',
                    'x': 'Dead',
                    'K': 'Wakekill',
                    'W': 'Waking',
                    'P': 'Parked',
                    'I': 'Idle'
                };
                const baseState = stat[0];
                const modifiers = stat.slice(1);
                let state = stateMap[baseState] || baseState;
                if (modifiers.includes('s')) state += ' (Session Leader)';
                if (modifiers.includes('l')) state += ' (Multi-threaded)';
                if (modifiers.includes('+')) state += ' (Foreground)';
                if (modifiers.includes('<')) state += ' (High Priority)';
                if (modifiers.includes('N')) state += ' (Low Priority)';

                return {
                    pid: parseInt(pid),
                    cpu: parseFloat(cpu),
                    memory: parseFloat(mem) * 1024 * 1024, // Memory in bytes
                    name,
                    state,
                    command
                };
            })
            .sort((a, b) => b.cpu - a.cpu)
            .slice(0, 10); // Get top 10 processes

        // Get total system metrics
        const { stdout: memInfo } = await execPromise('free -b | grep Mem');
        const totalMemory = parseInt(memInfo.split(/\s+/)[1]);
        const usedMemory = parseInt(memInfo.split(/\s+/)[2]);

        const { stdout: cpuInfo } = await execPromise("top -bn1 | grep 'Cpu(s)' | awk '{print $2 + $4}'");
        const cpuUsage = parseFloat(cpuInfo);

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
                    memory: p.memory // Memory in bytes
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
                const { stdout } = await execPromise(`ps -p ${pid} -o pid,pcpu,pmem,comm,stat,command`);
                const lines = stdout.split('\n').slice(1).filter(line => line.trim());
                if (lines.length > 0) {
                    const [pid, cpu, mem, comm, stat, ...cmdParts] = lines[0].trim().split(/\s+/);
                    const command = cmdParts.join(' ');
                    let name = command.split('/').pop().split(' ')[0];
                    if (name === '') name = comm;
                    
                    trackedProcessesData.push({
                        pid: parseInt(pid),
                        cpu: parseFloat(cpu),
                        memory: parseFloat(mem) * 1024 * 1024,
                        name,
                        state: stat,
                        command
                    });
                } else {
                    // Process doesn't exist anymore, mark for removal
                    processesToRemove.add(pid);
                }
            } catch (error) {
                // Process doesn't exist or error occurred, mark for removal
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

// Drop down menu options ------------------------------------------------------------
async function handleProcessAction(action, pid) {
    try {
        switch (action) {
            case 'kill':
                await execPromise(`kill -9 ${pid}`);
                break;
            case 'pause':
                await execPromise(`kill -STOP ${pid}`);
                break;
            case 'resume':
                await execPromise(`kill -CONT ${pid}`);
                break;
            case 'priority':
                // lesa NOT IMPLEMENTED YET
                break;
        }
    } catch (error) {
        console.error(`Error performing action ${action} on process ${pid}:`, error);
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