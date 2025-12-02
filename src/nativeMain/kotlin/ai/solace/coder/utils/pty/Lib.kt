// port-lint: source utils/pty/src/lib.rs
package ai.solace.coder.utils.pty

import ai.solace.coder.core.error.CodexResult
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.channels.ReceiveChannel
import kotlinx.coroutines.channels.SendChannel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.atomicfu.atomic
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.IO

import ai.solace.coder.core.createPlatformProcess
import ai.solace.coder.core.ProcessHandle
import ai.solace.coder.core.killPlatformChildProcessGroup
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.channels.ReceiveChannel
import kotlinx.coroutines.channels.SendChannel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.atomicfu.atomic
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.IO
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive

/**
 * Represents an active execution session.
 * In Rust this uses portable_pty, here we use pipes via ProcessExecutor/PlatformProcess.
 */
class ExecCommandSession(
    private val writerTx: SendChannel<ByteArray>,
    private val outputTx: ReceiveChannel<ByteArray>, // Broadcast in Rust, here simple channel or flow
    private val killer: ProcessKiller,
    private val readerJob: Job,
    private val writerJob: Job,
    private val waitJob: Job,
    private val exitStatus: kotlinx.atomicfu.AtomicBoolean,
    private val exitCode: kotlinx.atomicfu.AtomicInt
) {
    // Rust uses broadcast channel for output, allowing multiple subscribers.
    // We'll expose a flow or similar mechanism.
    // For parity with Rust's `output_receiver() -> broadcast::Receiver`, we might need a broadcast channel.
    // But for now, let's assume single consumer or handle it in the manager.
    
    // In Rust: pub fn writer_sender(&self) -> mpsc::Sender<Vec<u8>>
    fun writerSender(): SendChannel<ByteArray> = writerTx

    // In Rust: pub fn output_receiver(&self) -> broadcast::Receiver<Vec<u8>>
    // We'll return the channel for now, or a flow.
    // The Rust code subscribes to the broadcast channel.
    fun outputReceiver(): ReceiveChannel<ByteArray> = outputTx

    fun hasExited(): Boolean = exitStatus.value

    fun exitCode(): Int? {
        val code = exitCode.value
        return if (code == -1 && !hasExited()) null else code
    }
    
    fun kill() {
        killer.kill()
    }
}

interface ProcessKiller {
    fun kill()
}

data class SpawnedPty(
    val session: ExecCommandSession,
    val outputRx: ReceiveChannel<ByteArray>,
    val exitRx: ReceiveChannel<Int>
)

suspend fun spawnPtyProcess(
    command: String,
    args: List<String>,
    cwd: String,
    env: Map<String, String>,
    scope: CoroutineScope
): SpawnedPty {
    val process = createPlatformProcess(command, args, cwd, env)
    
    val writerCh = Channel<ByteArray>(Channel.UNLIMITED)
    val outputCh = Channel<ByteArray>(Channel.UNLIMITED)
    val exitCh = Channel<Int>(Channel.BUFFERED)
    
    val exitStatus = atomic(false)
    val exitCode = atomic(-1)
    
    val killer = object : ProcessKiller {
        override fun kill() {
            killPlatformChildProcessGroup(process)
        }
    }
    
    // Reader Jobs (Stdout/Stderr)
    // We launch separate jobs for stdout and stderr because read() is blocking
    val stdoutJob = scope.launch(Dispatchers.IO) {
        val buffer = ByteArray(4096)
        while (isActive) {
            val n = process.readStdout(buffer)
            if (n > 0) {
                outputCh.send(buffer.copyOfRange(0, n))
            } else {
                break
            }
        }
    }

    val stderrJob = scope.launch(Dispatchers.IO) {
        val buffer = ByteArray(4096)
        while (isActive) {
            val n = process.readStderr(buffer)
            if (n > 0) {
                outputCh.send(buffer.copyOfRange(0, n))
            } else {
                break
            }
        }
    }

    // Waiter Job: waits for process exit and ensures streams are drained
    val waiterJob = scope.launch(Dispatchers.IO) {
        val code = process.onAwait()
        exitCode.value = code
        exitStatus.value = true
        exitCh.send(code)
        
        // Wait for readers to finish draining
        stdoutJob.join()
        stderrJob.join()
        outputCh.close()
    }
    
    val writerJob = scope.launch(Dispatchers.IO) {
        for (data in writerCh) {
            // TODO: Implement writing to process stdin
        }
    }
    
    val waitJob = scope.launch(Dispatchers.IO) {
        // Already handled in readerJob for now
    }
    
    val session = ExecCommandSession(
        writerTx = writerCh,
        outputTx = outputCh,
        killer = killer,
        readerJob = stdoutJob, // Just a handle, waiterJob manages lifecycle
        writerJob = writerJob,
        waitJob = waiterJob,
        exitStatus = exitStatus,
        exitCode = exitCode
    )
    
    return SpawnedPty(session, outputCh, exitCh)
}
