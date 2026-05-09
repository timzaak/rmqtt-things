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
