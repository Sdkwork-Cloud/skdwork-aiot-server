const api = require('../../services/aiot-api');

Page({
  data: {
    devices: [],
  },
  onShow() {
    api.listDevices().then((response) => {
      const items = response.data?.items ?? [];
      this.setData({ devices: Array.isArray(items) ? items : [] });
    }).catch((error) => {
      wx.showToast({
        title: error instanceof Error ? error.message : '设备列表加载失败',
        icon: 'none',
      });
      this.setData({ devices: [] });
    });
  },
});
