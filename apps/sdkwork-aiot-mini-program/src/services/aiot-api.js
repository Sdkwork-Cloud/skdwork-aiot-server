const { getAiotAppSdkClient } = require('../sdk/aiot-app-sdk-client');

function getClient() {
  return getAiotAppSdkClient();
}

module.exports = {
  listDevices() {
    return getClient().iot.devicesList();
  },
  sendSpeakCommand(deviceId, text) {
    return getClient().iot.devicesCommandsCreate(deviceId, {
      capabilityName: 'audio.playback',
      commandName: 'speak',
      payload: { text, lang: 'zh-CN' },
    });
  },
  sendAgentChat(deviceId, text, sessionId) {
    return getClient().iot.devicesCommandsCreate(deviceId, {
      capabilityName: 'assistant',
      commandName: 'chat',
      payload: { text, lang: 'zh-CN' },
      ...(sessionId ? { sessionId } : {}),
    });
  },
  pollCommandResult(deviceId, commandId) {
    return getClient().iot.pollCommandResult(deviceId, commandId);
  },
};
