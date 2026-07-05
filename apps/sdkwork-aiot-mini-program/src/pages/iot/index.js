const api = require('../../services/aiot-api');

Page({
  data: { nodes: [] },
  onShow() {
    api.listDevices().then((response) => {
      const items = response.data?.items ?? [];
      this.setData({ nodes: Array.isArray(items) ? items : [] });
    }).catch(() => this.setData({ nodes: [] }));
  },
});
