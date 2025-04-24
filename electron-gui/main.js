const { app, BrowserWindow, ipcMain } = require('electron');
const path = require('path');
const { exec } = require('child_process');
const util = require('util');
const execPromise = util.promisify(exec);

let mainWindow;
let updateInterval;

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
                const stateMap = { // because the state is given in a weird format (letters)
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
                    memory: parseFloat(mem) * 1024 * 1024, // Convert to bytes
                    name,
                    state,
                    command
                };
            })
            .sort((a, b) => b.cpu - a.cpu) // Sorting by CPU usage
            .slice(0, 20); // To get the top 20 processes

        return processes;
    } catch (error) {
        console.error('Error getting processes:', error);
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
            mainWindow.webContents.send('process-update', processes);
        }
    }, 2000); // to update every 2 seconds
    ipcMain.on('process-action', async (event, { action, pid }) => {     // Handle Inter Process Communication
        await handleProcessAction(action, pid);
    });
    ipcMain.on('refresh-processes', async () => {
        const processes = await getProcesses();
        mainWindow.webContents.send('process-update', processes);
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