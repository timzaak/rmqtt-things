/**
 * Shared Axios client for API calls.
 */
import axios from 'axios'
import { handle401 } from '@/lib/auth'

const apiClient = axios.create({
  baseURL: '/api',
  withCredentials: true,
  headers: {
    'Content-Type': 'application/json',
  },
})

apiClient.interceptors.response.use(
  response => response,
  error => {
    if (error.response?.status === 401) {
      handle401()
    }

    return Promise.reject(error)
  },
)

export default apiClient
