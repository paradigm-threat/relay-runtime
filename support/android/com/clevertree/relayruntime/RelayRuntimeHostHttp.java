package com.clevertree.relayruntime;

/**
 * HTTP bridge for CleverScript {@code fetch()} on Android.
 * <p>
 * Copy into relay-client-android (or any app embedding relay-runtime JNI).
 * Replace {@link #hostHttpFetchSync} with OkHttp (or similar) and return the same JSON shape as web
 * {@code __relay_host_fetch}: {@code {"ok":bool,"status":number,"text":string,"error"?:string}}.
 */
public final class RelayRuntimeHostHttp {
    private RelayRuntimeHostHttp() {}

    /**
     * Synchronous HTTP fetch from Rust {@code AndroidHost::fetch}.
     *
     * @param url          request URL (absolute)
     * @param optionsJson  JSON object, e.g. {@code {"method":"GET"}}
     * @return JSON response string for the CleverScript runtime
     */
    public static String hostHttpFetchSync(String url, String optionsJson) {
        return "{\"ok\":false,\"status\":0,\"text\":\"\",\"error\":\"RelayRuntimeHostHttp.hostHttpFetchSync not implemented; wire OkHttp in relay-client-android\"}";
    }
}
