/**
 * 集中式选择器定义
 *
 * 所有 E2E 测试的元素选择器集中管理在此文件中。
 * 当前端 UI 变更时，只需修改此文件即可。
 *
 * 选择器优先级：
 * 1. data-testid（最稳定，优先使用）
 * 2. Aria roles（语义化）
 * 3. 文本内容（兜底）
 *
 * 根据项目实际情况修改每个选择器。
 */

/**
 * 将属性 key 转为 kebab-case，用于构造动态 data-testid。
 *
 * 与前端 toKebabKey 对齐。权威实现：frontend/src/lib/utils.ts 的
 * toKebabKey（camelCase 边界插连字符、连续大写末尾插连字符、非字母数字
 * 折叠为单连字符、去首尾连字符、小写）。例：`colorTemp` -> `color-temp`。
 * StateConfigurationSection.tsx 尚持有一份等价私有副本（待迁移到共享实现），
 * 迁移完成前两处必须保持一致。
 *
 * 供 `SELECTORS.stateConfiguration.targetApplyButton(key)` 等动态选择器复用，
 * 测试文件不得再内联各自的转换实现。
 */
export function toKebabKey(key: string): string {
  return key
    .replace(/([a-z0-9])([A-Z])/g, '$1-$2')
    .replace(/([A-Z]+)([A-Z][a-z])/g, '$1-$2')
    .replace(/[^a-zA-Z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .toLowerCase()
}

export const SELECTORS = {
  /** 登录页选择器 */
  login: {
    container: '[data-testid="login-card"], [data-testid="login-container"]',
    title: '[data-testid="login-title"]',
    usernameInput: '[data-testid="email-input"]',
    emailInput: '[data-testid="email-input"]',
    passwordInput: '[data-testid="password-input"]',
    submitButton: '[data-testid="login-submit-button"]',
    errorMessage: '[data-testid="login-error-message"]',
  },

  /** Dashboard 页选择器 */
  dashboard: {
    container: '[data-testid="dashboard-container"]',
    heading: '[data-testid="dashboard-heading"]',
    welcomeMessage: '[data-testid="welcome-message"]',
  },

  /** 通用组件选择器 */
  common: {
    dialog: '[data-testid="dialog"]',
    dialogTitle: '[data-testid="dialog-title"]',
    dialogContent: '[data-testid="dialog-content"]',
    dialogCloseButton: '[data-testid="dialog-close-button"]',
    dialogCancelButton: '[data-testid="dialog-cancel-button"]',
    dialogSubmitButton: '[data-testid="dialog-submit-button"]',

    form: '[data-testid="form"]',
    formEmailInput: '[data-testid="email-input"]',
    formPasswordInput: '[data-testid="password-input"]',
    formNicknameInput: '[data-testid="nickname-input"]',
    formNameInput: '[data-testid="name-input"]',

    toast: '[data-testid="toast"], [data-sonner-toast]',
    toastMessage: '[data-testid="toast-message"], [data-sonner-toast] [data-description]',
    successMessage: '[data-testid="success-message"], [data-sonner-toast][data-type="success"]',
    errorMessage: '[data-testid="error-message"], [data-sonner-toast][data-type="error"]',

    loading: '[data-testid="loading"]',
    spinner: '[data-testid="spinner"]',
  },

  /** Valid Templates 页选择器 */
  validTemplates: {
    showEditButton: '[data-testid="template-show-edit-button"]',
    showBackLink: '[data-testid="template-show-back-link"]',
    createProductSelect: '[data-testid="template-create-product-select"]',
    createEventInput: '[data-testid="template-create-event-input"]',
    createDescriptionInput: '[data-testid="template-create-description-input"]',
    createSubmitButton: '[data-testid="template-create-submit-button"]',
    editProductInput: '[data-testid="template-edit-product-input"]',
    editEventInput: '[data-testid="template-edit-event-input"]',
    editDescriptionInput: '[data-testid="template-edit-description-input"]',
    editStatusSelect: '[data-testid="template-edit-status-select"]',
    editSubmitButton: '[data-testid="template-edit-submit-button"]',
  },

  /** OTA 页选择器 */
  ota: {
    showBackLink: '[data-testid="ota-show-back-link"]',
  },

  /** Alarm Rules page selectors */
  alarmRules: {
    // List page
    createButton: '[data-testid="alarm-rule-create-button"]',
    searchForm: '[data-testid="alarm-rule-search-form"]',
    table: '[data-testid="alarm-rule-table"]',
    enabledSwitch: (id: number) => `[data-testid="alarm-rule-enabled-switch-${id}"]`,
    deleteConfirmDialog: '[data-testid="delete-confirm-dialog"]',

    // Create/Edit form
    productSelect: '[data-testid="product-select"]',
    nameInput: '[data-testid="name-input"]',
    descriptionInput: '[data-testid="description-input"]',
    triggerTypeSelect: '[data-testid="trigger-type-select"]',
    propertyNameInput: '[data-testid="property-name-input"]',
    eventIdentifierInput: '[data-testid="event-identifier-input"]',
    conditionOperatorSelect: '[data-testid="condition-operator-select"]',
    conditionValueInput: '[data-testid="condition-value-input"]',
    conditionMinInput: '[data-testid="condition-min-input"]',
    conditionMaxInput: '[data-testid="condition-max-input"]',
    actionsEditor: '[data-testid="actions-editor"]',
    actionLevelSelect: (index: number) => `[data-testid="action-level-select-${index}"]`,
    actionMessageInput: (index: number) => `[data-testid="action-message-input-${index}"]`,
    actionUrlInput: (index: number) => `[data-testid="action-url-input-${index}"]`,
    actionRemoveButton: (index: number) => `[data-testid="action-remove-button-${index}"]`,
    addAlarmActionButton: '[data-testid="add-alarm-action-button"]',
    addWebhookActionButton: '[data-testid="add-webhook-action-button"]',
    throttleMinutesInput: '[data-testid="throttle-minutes-input"]',
    submitButton: '[data-testid="submit-button"]',
    cancelButton: '[data-testid="cancel-button"]',

    // Edit page disabled fields
    productInputDisabled: '[data-testid="product-input-disabled"]',
    triggerTypeInputDisabled: '[data-testid="trigger-type-input-disabled"]',
  },

  /** Alarm Records page selectors */
  alarms: {
    searchForm: '[data-testid="alarm-search-form"]',
    table: '[data-testid="alarm-table"]',
    statusTag: (id: number) => `[data-testid="alarm-status-tag-${id}"]`,
    ackButton: (id: number) => `[data-testid="ack-alarm-button-${id}"]`,
    clearButton: (id: number) => `[data-testid="clear-alarm-button-${id}"]`,
    statusFilter: '[data-testid="status-filter-select"]',
  },

  /** Products 页选择器 */
  products: {
    /** getByRole('link', { name: ... }) */
    createLink: 'Create Product',
    /** getByRole('button', { name: ... }) */
    createButton: 'Create',
    /** getByRole('button', { name: ... }) */
    saveButton: 'Save',
    /** getByRole('link', { name: ... }) */
    cancelButton: 'Cancel',
    /** getByLabel(...) */
    nameInput: 'Name',
    /** getByLabel(...) */
    modelNoInput: 'Model Number',
    /** getByLabel(...) */
    descriptionInput: 'Description',
    /** getByLabel(...) */
    searchInput: 'Search',
    /** getByRole('button', { name: ... }) */
    searchButton: 'Search',
    /** getByRole('link', { name: ... }) - Edit link in table rows */
    editLink: 'Edit',
    /** getByText(...) - Auto Provisioning label on product edit page */
    autoProvisioningLabel: 'Auto Provisioning',
    /** getByText(...) - Helper text under auto provisioning checkbox */
    autoProvisioningText: 'Enable device auto-provisioning for this product',
  },

  /** Devices page selectors */
  devices: {
    /** getByRole('columnheader', { name: ... }) - Registration column header */
    registrationColumnHeader: 'Registration',
    /** getByLabel(...) - Registration filter select */
    registrationFilterLabel: 'Registration',
    /** option text in the filter dropdown */
    registrationAutoOption: 'Auto',
    /** option text in the filter dropdown */
    registrationManualOption: 'Manual',
  },

  /**
   * @deprecated（校准确认已失效，待删除）
   *
   * Property Shadow 面板选择器 (US-PA-042/043/044)。
   *
   * 已失效：前端 `PropertyShadowSection.tsx` 已随 device-detail-experience
   * 七区改造删除，`shadow-section` / `shadow-delta-table` / `shadow-set-button` /
   * `shadow-desired-editor` 在 frontend/src 内 rg 均无匹配（校准证据）。
   * 功能由 `stateConfiguration` 组（StateConfigurationSection.tsx）接替。
   * 暂保留仅供编译过渡，新测试不得使用。
   */
  shadow: {
    section: '[data-testid="shadow-section"]',
    deltaTable: '[data-testid="shadow-delta-table"]',
    setButton: '[data-testid="shadow-set-button"]',
    desiredEditor: '[data-testid="shadow-desired-editor"]',
    submitButton: '[data-testid="shadow-submit-button"]',
    cancelButton: '[data-testid="shadow-cancel-button"]',
  },

  /**
   * @deprecated（校准确认已失效，待删除）
   *
   * Action Invocation 面板选择器（US-PA-048 / US-PA-049）。
   *
   * 面板级 testid 已失效：前端 `ActionInvocationsSection.tsx` 已删除，
   * `invokeButton`（action-invoke-button）与 `invocationTable`
   * （action-invocation-table）在 frontend/src 内 rg 均无匹配；入口与历史表由
   * `operations` 组（`runActionButton` / `table`）接替。
   *
   * 对话框 testid 仍有效（ActionInvokeDialog.tsx 保留，校准证据：
   * `action-invoke-dialog` :57 / `service-type-input` :81 / `params-input` :114），
   * 但与 `operations.actionDialog` 等重复，新测试请使用 `operations` 组。
   * 暂保留仅供编译过渡。
   */
  actions: {
    invokeButton: '[data-testid="action-invoke-button"]',
    invokeDialog: '[data-testid="action-invoke-dialog"]',
    serviceTypeInput: '[data-testid="service-type-input"]',
    paramsInput: '[data-testid="params-input"]',
    invocationTable: '[data-testid="action-invocation-table"]',
    submitButton: '[data-testid="submit-button"]',
    cancelButton: '[data-testid="cancel-button"]',
  },

  /**
   * State & Configuration 分区选择器（US-PA-050/051）
   *
   * 对应组件 frontend/src/components/device-detail/StateConfigurationSection.tsx。
   * 校准证据：`target-update-button`（:204）、
   * `state-configuration-table`（:222）、`target-apply-button-${kebabKey}`
   * （:180，key 经组件内 toKebabKey :36-42 转换，与上方导出 helper 同规则）、
   * `target-update-dialog` / `target-json-input`（:292/:306）均存在于前端实现。
   * 对话框内 submit/cancel 复用通用 testid（:352/:333），使用时须限定在
   * updateDialog 作用域内。同步状态文案（:99-107）：In sync / Out of sync /
   * Target not set。
   */
  stateConfiguration: {
    table: '[data-testid="state-configuration-table"]',
    targetUpdateButton: '[data-testid="target-update-button"]',
    /** 动态构造：单属性 Apply 按钮，key 由 toKebabKey 转换（与前端规则一致） */
    targetApplyButton: (key: string) =>
      `[data-testid="target-apply-button-${toKebabKey(key)}"]`,
    updateDialog: '[data-testid="target-update-dialog"]',
    targetJsonInput: '[data-testid="target-json-input"]',
    /** 限定在 updateDialog 内使用 */
    dialogSubmitButton: '[data-testid="submit-button"]',
    /** 限定在 updateDialog 内使用 */
    dialogCancelButton: '[data-testid="cancel-button"]',
  },

  /**
   * Operations 分区选择器（US-PA-051）
   *
   * 对应组件（校准证据，均存在于前端实现）：
   * - frontend/src/components/device-detail/DeviceOperationsSection.tsx
   *   （typeFilter :106 / statusFilter :123 / runActionButton :140 /
   *   moreActionsButton :149 / directPropertyWriteButton :170 / table :205 /
   *   error :194）
   * - frontend/src/components/device-detail/ActionInvokeDialog.tsx
   *   （actionDialog :57 / actionServiceTypeInput :81 / actionParamsInput :114）
   * - frontend/src/components/device-detail/DirectPropertyWriteDialog.tsx
   *   （directWriteDialog :64 / directWriteJsonInput :78 /
   *   targetConflictWarning :101；固定警告文案 :8-9）
   *
   * 类型筛选 option value 为后端枚举：targetSync /
   * directPropertyWrite / actionInvocation；Type 列文案 Target sync /
   * Direct write / Action（DeviceOperationsSection.tsx:15-25）。
   * 对话框 submit/cancel 复用通用 testid，须限定在对应 dialog 作用域内。
   */
  operations: {
    table: '[data-testid="device-operations-table"]',
    error: '[data-testid="device-operations-error"]',
    typeFilter: '[data-testid="operation-type-filter"]',
    statusFilter: '[data-testid="operation-status-filter"]',
    runActionButton: '[data-testid="run-action-button"]',
    moreActionsButton: '[data-testid="more-actions-button"]',
    directPropertyWriteButton: '[data-testid="direct-property-write-button"]',
    actionDialog: '[data-testid="action-invoke-dialog"]',
    actionServiceTypeInput: '[data-testid="service-type-input"]',
    actionParamsInput: '[data-testid="params-input"]',
    directWriteDialog: '[data-testid="direct-property-write-dialog"]',
    directWriteJsonInput: '[data-testid="command-json-input"]',
    targetConflictWarning: '[data-testid="target-conflict-warning"]',
    /** 限定在对应 dialog 内使用 */
    dialogSubmitButton: '[data-testid="submit-button"]',
    /** 限定在对应 dialog 内使用 */
    dialogCancelButton: '[data-testid="cancel-button"]',
  },

  /**
   * 设备详情页 Tab 切换选择器（device-detail-experience 七区结构）
   *
   * 七区信息架构（frontend/src/routes/devices/show.$id.tsx TABS
   * :22-34）：Overview / State & Configuration / Operations / Reported Data /
   * Events / Connectivity / Metadata，Tab 为 role=tab 的 button。仅
   * State & Configuration、Operations、Reported Data 提供 data-testid，
   * 优先以 testid 定位（page.locator）；其余四个无 testid，
   * 以 getByRole('tab', { name }) 按稳定文案定位。
   *
   * 校准证据：旧 Shadow / Commands / Actions Tab 已随
   * PropertyShadowSection / PropertyCommandsSection / ActionInvocationsSection
   * 一并删除（frontend/src 内 rg 无匹配），对应旧 key 同步移除。
   */
  deviceTabs: {
    /** getByRole('tab', { name }) — Overview（默认激活，无 testid） */
    overviewTab: 'Overview',
    /** page.locator(...) — State & Configuration tab */
    stateConfigurationTab: '[data-testid="device-tab-state-configuration"]',
    /** page.locator(...) — Operations tab */
    operationsTab: '[data-testid="device-tab-operations"]',
    /** page.locator(...) — Reported Data tab */
    reportedDataTab: '[data-testid="device-tab-reported-data"]',
    /** getByRole('tab', { name }) — Events（无 testid） */
    eventsTab: 'Events',
    /** getByRole('tab', { name }) — Connectivity（无 testid） */
    connectivityTab: 'Connectivity',
    /** getByRole('tab', { name }) — Metadata（无 testid） */
    metadataTab: 'Metadata',
  },

  /**
   * 属性历史图表视图选择器（property-history-visualization）。
   *
   * 图表视图与既有表格并存于 Property History 区域（默认图表）。动态 key
   * 选择器经 `toKebabKey` 转换，如
   * `propertyHistoryChart.keyToggle('colorTemp')` →
   * `[data-testid="property-chart-key-toggle-color-temp"]`。
   */
  propertyHistoryChart: {
    /** page.locator(...) — Chart 视图切换按钮 */
    viewChartButton: '[data-testid="property-history-view-chart"]',
    /** page.locator(...) — Table 视图切换按钮 */
    viewTableButton: '[data-testid="property-history-view-table"]',
    /** 图表容器（含 canvas，悬停目标） */
    container: '[data-testid="property-chart-container"]',
    /** 时间范围下拉（page.locator + selectOptions） */
    rangeSelect: '[data-testid="property-chart-range-select"]',
    /** 自定义起止输入（page.locator + fill） */
    startInput: '[data-testid="property-chart-start-input"]',
    endInput: '[data-testid="property-chart-end-input"]',
    /** 空范围空态（所选范围内无数据） */
    empty: '[data-testid="property-chart-empty"]',
    /** 降精度标注（动态 key，需 toKebabKey） */
    downsampleNote: (key: string) =>
      `[data-testid="property-chart-downsample-note-${toKebabKey(key)}"]`,
    /** 属性标签（多选，兼图例；动态 key，需 toKebabKey） */
    keyToggle: (key: string) =>
      `[data-testid="property-chart-key-toggle-${toKebabKey(key)}"]`,
  },
}

/**
 * 选择器辅助：支持多备选选择器
 */
export function getSelector(selector: string | string[]): string {
  if (Array.isArray(selector)) {
    return selector.join(', ')
  }
  return selector
}
