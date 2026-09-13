package vendor.xiaomi.hardware.displayfeature_aidl;

@VintfStability
oneway interface IDisplayFeatureCallback {
    void displayfeatureInfoChanged(int eventId, int value, float arg2, float arg3, float arg4);
}

