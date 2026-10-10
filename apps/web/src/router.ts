/** Public browser surface: product story, working demo console and independent verifier. */
import { createRouter, createWebHistory } from 'vue-router'
import LandingPage from './pages/LandingPage.vue'
import { session } from './demo/workspace'

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
      path: '/chain-history/transportadora/gado',
      name: 'chain-carrier-livestock',
      component: () => import('./pages/chain/CarrierPage.vue'),
      props: { leg: 'livestock' },
    },
    {
      path: '/chain-history/transportadora/carne',
      name: 'chain-carrier-meat',
      component: () => import('./pages/chain/CarrierPage.vue'),
      props: { leg: 'meat' },
    },
    {
      path: '/chain-history/transportadora/entrega',
      name: 'chain-carrier-delivery',
      component: () => import('./pages/chain/CarrierPage.vue'),
      props: { leg: 'delivery' },
    },
    // Former single carrier page; kept so links shared before the export leg still resolve.
    { path: '/chain-history/transportadora', redirect: '/chain-history/transportadora/carne' },
    {
      path: '/chain-history/exportador',
      name: 'chain-exporter',
      component: () => import('./pages/chain/ExporterPage.vue'),
    },
    {
      path: '/chain-history/mercado',
      name: 'chain-market',
      component: () => import('./pages/chain/MarketPage.vue'),
    },
    { path: '/login', name: 'login', component: () => import('./pages/LoginPage.vue') },
    {
      path: '/painel',
      name: 'workspace',
      component: () => import('./pages/WorkspacePage.vue'),
      // Simulated access: the workspace needs a demo profile chosen on the login screen.
      beforeEnter: () => (session.value ? true : { name: 'login' }),
    },
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
