<script setup lang="ts">
import { onMounted, onUnmounted, ref, watch } from 'vue'
import { Terminal } from '@xterm/xterm'
import { FitAddon } from '@xterm/addon-fit'
import { WebLinksAddon } from '@xterm/addon-web-links'
import { useSessionStore } from '../stores/session'
import { createTerminalWebSocket } from '../api/client'
import '@xterm/xterm/css/xterm.css'

const terminalRef = ref<HTMLDivElement>()
const session = useSessionStore()

let term: Terminal | null = null
let fitAddon: FitAddon | null = null
let ws: WebSocket | null = null
let currentLine = ''

function initTerminal() {
  if (!terminalRef.value || !session.id) return

  // Create terminal
  term = new Terminal({
    theme: {
      background: '#0f0f23',
      foreground: '#e8e8e8',
      cursor: '#00d4ff',
      cursorAccent: '#0f0f23',
      selectionBackground: '#7c3aed44',
      black: '#1a1a2e',
      red: '#ef4444',
      green: '#10b981',
      yellow: '#f59e0b',
      blue: '#3b82f6',
      magenta: '#7c3aed',
      cyan: '#00d4ff',
      white: '#e8e8e8',
    },
    fontFamily: '"Fira Code", "Cascadia Code", Menlo, Monaco, monospace',
    fontSize: 14,
    lineHeight: 1.2,
    cursorBlink: true,
    cursorStyle: 'bar',
  })

  // Add addons
  fitAddon = new FitAddon()
  term.loadAddon(fitAddon)
  term.loadAddon(new WebLinksAddon())

  // Open terminal
  term.open(terminalRef.value)
  fitAddon.fit()

  // Connect WebSocket
  connectWebSocket()

  // Handle resize
  const resizeObserver = new ResizeObserver(() => {
    fitAddon?.fit()
  })
  resizeObserver.observe(terminalRef.value)

  // Handle input
  term.onKey(({ key, domEvent }) => {
    if (!ws || ws.readyState !== WebSocket.OPEN) return

    if (domEvent.key === 'Enter') {
      term?.write('\r\n')
      if (currentLine.trim()) {
        ws.send(JSON.stringify({ type: 'Execute', command: currentLine }))
      } else {
        ws.send(JSON.stringify({ type: 'Execute', command: '' }))
      }
      currentLine = ''
    } else if (domEvent.key === 'Backspace') {
      if (currentLine.length > 0) {
        currentLine = currentLine.slice(0, -1)
        term?.write('\b \b')
      }
    } else if (domEvent.key === 'Tab') {
      domEvent.preventDefault()
      ws.send(JSON.stringify({ type: 'GetCompletions', prefix: currentLine }))
    } else if (domEvent.ctrlKey && domEvent.key === 'c') {
      currentLine = ''
      term?.write('^C\r\n')
      ws.send(JSON.stringify({ type: 'Execute', command: '' }))
    } else if (domEvent.ctrlKey && domEvent.key === 'l') {
      term?.clear()
    } else if (key.length === 1 && !domEvent.ctrlKey && !domEvent.altKey) {
      currentLine += key
      term?.write(key)
    }
  })

  // Handle paste
  term.onData((data) => {
    // Only handle paste (multi-character input)
    if (data.length > 1) {
      currentLine += data
      term?.write(data)
    }
  })
}

function connectWebSocket() {
  if (!session.id) return

  ws = createTerminalWebSocket(session.id)

  ws.onopen = () => {
    console.log('Terminal WebSocket connected')
  }

  ws.onmessage = (event) => {
    try {
      const response = JSON.parse(event.data)
      handleResponse(response)
    } catch (e) {
      console.error('Failed to parse WebSocket message:', e)
    }
  }

  ws.onerror = (error) => {
    console.error('Terminal WebSocket error:', error)
    term?.writeln('\r\n\x1b[31mConnection error\x1b[0m')
  }

  ws.onclose = () => {
    console.log('Terminal WebSocket closed')
    term?.writeln('\r\n\x1b[33mDisconnected. Refresh to reconnect.\x1b[0m')
  }
}

function handleResponse(response: any) {
  switch (response.type) {
    case 'Output':
      if (response.format === 'ansi') {
        term?.write(response.text)
      } else {
        term?.write(response.text.replace(/\n/g, '\r\n'))
      }
      break
    case 'Prompt':
      term?.write(`\r\n\x1b[36m${response.text}\x1b[0m`)
      break
    case 'Completions':
      if (response.items.length > 0) {
        term?.writeln('')
        term?.writeln(response.items.join('  '))
        term?.write(`\x1b[36mormdb> \x1b[0m${currentLine}`)
      }
      break
    case 'Error':
      term?.write(`\r\n\x1b[31mError: ${response.message}\x1b[0m`)
      break
  }
}

onMounted(() => {
  if (session.id) {
    initTerminal()
  }
})

watch(() => session.id, (newId) => {
  if (newId && !term) {
    initTerminal()
  }
})

onUnmounted(() => {
  ws?.close()
  term?.dispose()
})
</script>

<template>
  <div class="terminal-container">
    <div ref="terminalRef" class="terminal"></div>
    <div v-if="!session.id" class="terminal-placeholder">
      <p>Creating session...</p>
    </div>
  </div>
</template>

<style scoped>
.terminal-container {
  height: 100%;
  background: #0f0f23;
  padding: 0.5rem;
}

.terminal {
  height: 100%;
}

.terminal-placeholder {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  color: var(--color-text-secondary);
}
</style>
