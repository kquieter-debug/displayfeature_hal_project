package vendor.xiaomi.hardware.displayfeature_aidl;

import vendor.xiaomi.hardware.displayfeature_aidl.IDisplayFeatureCallback;

@VintfStability
interface IDisplayFeature {
    oneway void notifyBrightness(int brightness);
    void registerCallback(int unused, IDisplayFeatureCallback callback);
    oneway void sendMessage(int packedMessageId, int value, String text);
    void sendPanelCommand(String command);
    void sendPostProcCommand(int command, int value);
    void sendRefreshCommand();
    void setFeature(int displayId, int caseId, int modeId, int cookie);
    void setFunction(int displayId, int caseId, int modeId, int cookie);
    void sendGamePkgName(int displayId, int caseId, int modeId, String packageName);
}

