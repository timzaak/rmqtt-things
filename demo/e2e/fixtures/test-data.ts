/**
 * 测试数据管理
 *
 * 集中管理 E2E 测试使用的测试数据和辅助函数。
 * 修改此文件以匹配项目实际数据需求。
 */

export { DEMO_ADMIN } from '../helpers/auth'

export interface TestAccount {
  email: string
  password: string
}

export interface TestDevice {
  id: string
  name: string
  productKey?: string
}

export const TEST_DEVICE: TestDevice = {
  id: 'demo-device',
  name: 'Demo Device',
}

/**
 * 生成随机测试设备数据
 */
export function generateTestDevice(options?: {
  id?: string
  name?: string
  productKey?: string
}): TestDevice {
  const timestamp = Date.now()
  const random = Math.floor(Math.random() * 1000)

  return {
    id: options?.id || `test-device-${timestamp}-${random}`,
    name: options?.name || `Test Device ${timestamp}`,
    productKey: options?.productKey,
  }
}
