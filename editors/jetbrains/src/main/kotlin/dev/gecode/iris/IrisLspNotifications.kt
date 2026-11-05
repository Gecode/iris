package dev.gecode.iris

import com.intellij.notification.Notification
import com.intellij.notification.NotificationAction
import com.intellij.notification.NotificationGroupManager
import com.intellij.notification.NotificationType
import com.intellij.openapi.components.Service
import com.intellij.openapi.options.ShowSettingsUtil
import com.intellij.openapi.progress.ProgressIndicator
import com.intellij.openapi.progress.Task
import com.intellij.openapi.project.Project
import com.intellij.openapi.util.text.StringUtil
import com.intellij.platform.lsp.api.LspServerManager
import java.util.concurrent.atomic.AtomicBoolean

@Service(Service.Level.PROJECT)
class IrisLspNotifications(private val project: Project) {
    private var missingExecutable: Notification? = null
    private val downloading = AtomicBoolean()

    @Synchronized
    internal fun showMissingExecutable(release: IrisBinaryDownload? = null) {
        if (missingExecutable?.isExpired == false) return
        val offer = when {
            release == null -> ""
            release.target == null -> "<br>No Iris release is available for ${release.os} (${release.arch}). Use a manual installation."
            else -> "<br>Download iris ${release.version} for ${release.target} from ${release.releaseUrl}? " +
                "The IDE reports the transfer size during download. No files are downloaded until you accept."
        }
        val notification = notificationGroup()
            .createNotification(
                "Iris executable not found",
                "Install iris by building from the Iris repository: in rust/, run " +
                    "<code>cargo build --locked -p gecode-iris-cli --bin iris</code>. " +
                    "In Settings → Languages &amp; Frameworks → Iris, set Executable path to " +
                    "rust/target/debug/iris (iris.exe on Windows), or put iris on PATH and leave the field empty. " +
                    "Then run Restart Iris Language Server. Check that a configured file is executable." + offer,
                NotificationType.WARNING,
            ).addAction(NotificationAction.createSimple("Configure Iris") {
                ShowSettingsUtil.getInstance().showSettingsDialog(project, IrisLspConfigurable::class.java)
            })
        if (release?.target != null) {
            notification.addAction(NotificationAction.createSimple("Download iris ${release.version}") { installServer(release) })
            // Keep the warning and manual-install instructions available after declining.
            notification.addAction(NotificationAction.createSimple("Not now") { notification.hideBalloon() })
        }
        missingExecutable = notification
        notification.notify(project)
    }

    private fun installServer(release: IrisBinaryDownload) {
        if (!downloading.compareAndSet(false, true)) return
        object : Task.Backgroundable(project, "Download iris ${release.version}", true) {
            override fun run(indicator: ProgressIndicator) { release.install(indicator) }

            override fun onSuccess() {
                if (project.isDisposed) return
                clear()
                LspServerManager.getInstance(project).stopAndRestartIfNeeded(IrisLspSupportProvider::class.java)
            }

            override fun onThrowable(error: Throwable) {
                if (project.isDisposed) return
                notificationGroup().createNotification(
                    "Iris download failed",
                    StringUtil.escapeXmlEntities(error.message ?: error.javaClass.simpleName) +
                        " Retry from the Iris executable not found notification, or configure a manually installed binary.",
                    NotificationType.ERROR,
                ).notify(project)
            }

            override fun onFinished() { downloading.set(false) }
        }.queue()
    }

    private fun notificationGroup() = NotificationGroupManager.getInstance().getNotificationGroup("Iris language server")

    @Synchronized
    fun clear() {
        missingExecutable?.expire()
        missingExecutable = null
    }
}
