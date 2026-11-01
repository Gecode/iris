package org.gecode.iris

import com.intellij.notification.Notification
import com.intellij.notification.NotificationAction
import com.intellij.notification.NotificationGroupManager
import com.intellij.notification.NotificationType
import com.intellij.openapi.components.Service
import com.intellij.openapi.options.ShowSettingsUtil
import com.intellij.openapi.project.Project

@Service(Service.Level.PROJECT)
class IrisLspNotifications(private val project: Project) {
    private var missingExecutable: Notification? = null

    @Synchronized
    fun showMissingExecutable() {
        if (missingExecutable?.isExpired == false) return
        missingExecutable = NotificationGroupManager.getInstance()
            .getNotificationGroup("Iris language server")
            .createNotification(
                "Iris executable not found",
                "Install iris by building from the Iris repository: in rust/, run " +
                    "<code>cargo build --locked -p iris-cli --bin iris</code>. " +
                    "In Settings → Languages &amp; Frameworks → Iris, set Executable path to " +
                    "rust/target/debug/iris (iris.exe on Windows), or put iris on PATH and leave the field empty. " +
                    "Then run Restart Iris Language Server. Check that a configured file is executable.",
                NotificationType.WARNING,
            ).addAction(NotificationAction.createSimple("Configure Iris") {
                ShowSettingsUtil.getInstance().showSettingsDialog(project, IrisLspConfigurable::class.java)
            }).also { it.notify(project) }
    }

    @Synchronized
    fun clear() {
        missingExecutable?.expire()
        missingExecutable = null
    }
}
