/** Public browser surface: product story, working demo console and independent verifier. */
import { createRouter, createWebHistory } from 'vue-router'
import LandingPage from './pages/LandingPage.vue'

export const router = createRouter({
  history: createWebHistory(),
  routes: [
    { path: '/', name: 'landing', component: LandingPage },
    { path: '/problem', name: 'problem', component: () => import('./pages/ProblemPage.vue') },
    { path: '/future', name: 'future', component: () => import('./pages/FuturePage.vue') },
    { path: '/demo', name: 'demo', component: () => import('./pages/DemoPage.vue') },
    {
      path: '/lineage/:assetId?',
      name: 'lineage',
      component: () => import('./pages/LineagePage.vue'),
    },
    {
      path: '/operations',
      name: 'operations',
      component: () => import('./pages/OperationsPage.vue'),
    },
    {
      path: '/verify/:assetId?',
      name: 'verify',
      component: () => import('./pages/VerifyPage.vue'),
    },
  ],
})
