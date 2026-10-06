/** Public browser surface: product story, working demo console and independent verifier. */
import { createRouter, createWebHistory } from 'vue-router'
import LandingPage from './pages/LandingPage.vue'

export const router = createRouter({
  history: createWebHistory(),
  routes: [
    { path: '/', name: 'landing', component: LandingPage },
    { path: '/problem', name: 'problem', component: () => import('./pages/ProblemPage.vue') },
    { path: '/future', name: 'future', component: () => import('./pages/FuturePage.vue') },
    {
      path: '/chain-history',
      name: 'chain-history',
      component: () => import('./pages/ChainHistoryPage.vue'),
    },
    {
      path: '/chain-history/produtor',
      name: 'chain-producer',
      component: () => import('./pages/chain/ProducerPage.vue'),
    },
    {
      path: '/chain-history/produtor/animal',
      name: 'chain-producer-animal',
      component: () => import('./pages/chain/ProducerAnimalPage.vue'),
    },
    {
      path: '/chain-history/produtor/propriedade',
      name: 'chain-producer-property',
      component: () => import('./pages/chain/ProducerPropertyPage.vue'),
    },
    {
      path: '/chain-history/frigorifico',
      name: 'chain-slaughterhouse',
      component: () => import('./pages/chain/SlaughterhousePage.vue'),
    },
    {
      path: '/chain-history/transportadora',
      name: 'chain-carrier',
      component: () => import('./pages/chain/CarrierPage.vue'),
    },
    {
      path: '/chain-history/mercado',
      name: 'chain-market',
      component: () => import('./pages/chain/MarketPage.vue'),
    },
    { path: '/login', name: 'login', component: () => import('./pages/LoginPage.vue') },
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
