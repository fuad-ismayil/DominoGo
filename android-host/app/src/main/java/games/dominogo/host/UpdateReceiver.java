package games.dominogo.host;

import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;
import android.content.pm.PackageInstaller;
import android.os.Build;

/**
 * Installer results and the post-update restart.
 *
 * INSTALL_RESULT comes only from our own PackageInstaller session (explicit,
 * non-exported component). MY_PACKAGE_REPLACED is a protected system broadcast
 * delivered only to this package after it has been updated.
 */
public class UpdateReceiver extends BroadcastReceiver {
    static final String INSTALL_RESULT = "games.dominogo.host.INSTALL_RESULT";

    @Override public void onReceive(Context context, Intent intent) {
        String action = intent.getAction();
        if (Intent.ACTION_MY_PACKAGE_REPLACED.equals(action)) {
            String target = ApkUpdater.takeResume(context);
            ApkUpdater.writeState(context, "Updated to Go Host " + BuildConfig.VERSION_NAME + "."
                + (target.isEmpty() ? "" : " Hosting restarted automatically."), false);
            if (!target.isEmpty()) startHosting(context, target);
            return;
        }
        if (!INSTALL_RESULT.equals(action)) return;
        int status = intent.getIntExtra(PackageInstaller.EXTRA_STATUS, PackageInstaller.STATUS_FAILURE);
        if (status == PackageInstaller.STATUS_PENDING_USER_ACTION) {
            // First self-update (or Android policy): the system asks the user to confirm.
            Intent confirm = Build.VERSION.SDK_INT >= 33
                ? intent.getParcelableExtra(Intent.EXTRA_INTENT, Intent.class)
                : legacyIntent(intent);
            if (confirm != null) {
                ApkUpdater.writeState(context, "Confirm the update in Android's installer.", false);
                context.startActivity(confirm.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK));
            }
            return;
        }
        if (status == PackageInstaller.STATUS_SUCCESS) return; // MY_PACKAGE_REPLACED follows in the new process.
        String detail = intent.getStringExtra(PackageInstaller.EXTRA_STATUS_MESSAGE);
        String message = status == PackageInstaller.STATUS_FAILURE_ABORTED ? "Update canceled. The installed version is still in use."
            : "Update could not be installed" + (detail == null || detail.isEmpty() ? "." : ": " + detail);
        ApkUpdater.writeState(context, message, status != PackageInstaller.STATUS_FAILURE_ABORTED);
        // The games were stopped for the update; bring back what was running.
        String target = ApkUpdater.takeResume(context);
        if (!target.isEmpty()) startHosting(context, target);
    }

    @SuppressWarnings("deprecation")
    private static Intent legacyIntent(Intent intent) { return intent.getParcelableExtra(Intent.EXTRA_INTENT); }

    private static void startHosting(Context context, String target) {
        try {
            context.startForegroundService(new Intent(context, HostService.class).setAction(HostService.START).putExtra("game", target));
        } catch (RuntimeException e) {
            ApkUpdater.writeState(context, "Updated, but Android blocked the automatic restart. Open Go Host and tap Start.", true);
        }
    }
}
