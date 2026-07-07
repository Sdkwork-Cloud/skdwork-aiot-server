const api = require('../../services/aiot-api');

Page({
  data: {
    devices: [],
    draft: '',
    selectedDeviceId: '',
  },
  onShow() {
    api.listDevices().then((response) => {
      const devices = response.data?.items ?? [];
      const selectedDeviceId = devices[0] ? (devices[0].deviceId || devices[0].id) : '';
      this.setData({
        devices: Array.isArray(devices) ? devices : [],
        selectedDeviceId,
      });
    }).catch((error) => {
      wx.showToast({
        title: error instanceof Error ? error.message : '设备列表加载失败',
        icon: 'none',
      });
    });
  },
  onInput(event) {
    this.setData({ draft: event.detail.value });
  },
  onDeviceChange(event) {
    const index = Number(event.detail.value);
    const device = this.data.devices[index];
    const selectedDeviceId = device ? (device.deviceId || device.id) : '';
    this.setData({ selectedDeviceId });
  },
  onSpeak() {
    const text = this.data.draft.trim();
    if (!text || !this.data.selectedDeviceId) return;
    api.sendSpeakCommand(this.data.selectedDeviceId, text).then(() => {
      wx.showToast({ title: '已发送语音命令', icon: 'success' });
      this.setData({ draft: '' });
    }).catch((error) => {
      wx.showToast({
        title: error instanceof Error ? error.message : '语音命令发送失败',
        icon: 'none',
      });
    });
  },
});
