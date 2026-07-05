const api = require('../../services/aiot-api');

Page({
  data: {
    devices: [],
    draft: '',
    selectedDeviceId: '',
    messages: [],
    sessionId: '',
    isSending: false,
  },
  onShow() {
    api.listDevices().then((response) => {
      const devices = response.data?.items ?? [];
      const selectedDeviceId = devices[0] ? (devices[0].deviceId || devices[0].id) : '';
      this.setData({
        devices: Array.isArray(devices) ? devices : [],
        selectedDeviceId,
        sessionId: selectedDeviceId ? `agent-${selectedDeviceId}` : '',
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
    this.setData({
      selectedDeviceId,
      sessionId: selectedDeviceId ? `agent-${selectedDeviceId}` : '',
    });
  },
  onSend() {
    const text = this.data.draft.trim();
    if (!text || !this.data.selectedDeviceId || this.data.isSending) {
      return;
    }

    const userMessage = {
      id: `user-${Date.now()}`,
      role: 'user',
      content: text,
    };
    const nextMessages = [...this.data.messages, userMessage];
    this.setData({ messages: nextMessages, draft: '', isSending: true });

    api.sendAgentChat(this.data.selectedDeviceId, text, this.data.sessionId)
      .then(async (response) => {
        const commandId = response.data?.commandId;
        if (!commandId) {
          throw new Error('命令未接受');
        }
        const completed = await api.pollCommandResult(this.data.selectedDeviceId, commandId);
        if (!completed) {
          throw new Error('智能体响应超时');
        }
        const result = completed.resultPayload;
        const assistantText = typeof result === 'object' && result
          ? (result.text || result.reply || result.message || JSON.stringify(result))
          : String(result ?? '智能体已完成处理');
        this.setData({
          messages: [
            ...nextMessages,
            {
              id: `assistant-${Date.now()}`,
              role: 'assistant',
              content: String(assistantText),
            },
          ],
          isSending: false,
        });
      })
      .catch((error) => {
        wx.showToast({
          title: error instanceof Error ? error.message : '智能体消息发送失败',
          icon: 'none',
        });
        this.setData({ isSending: false });
      });
  },
});
