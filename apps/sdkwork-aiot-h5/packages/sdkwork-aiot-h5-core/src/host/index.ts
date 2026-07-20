export interface AiotH5HostCapabilities {
  readonly browser: boolean;
  readonly capacitor: boolean;
}

export const aiotH5BrowserHost: AiotH5HostCapabilities = Object.freeze({
  browser: true,
  capacitor: false,
});
