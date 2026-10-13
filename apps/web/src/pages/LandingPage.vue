<script setup lang="ts">
import { computed, ref } from 'vue'
import { RouterLink, useRouter } from 'vue-router'
import SystemFlow from '../components/flow/SystemFlow.vue'
import { session } from '../demo/workspace'
import { useCopy } from '../i18n'

const router = useRouter()
const assetId = ref('')

const copy = useCopy({
  en: {
    kicker: 'Cattle traceability',
    title: 'Verifiable identity and custody for physical livestock.',
    support: ['Physical identifiers can change.', 'Verifiable identity and custody should not.'],
    demo: 'Open Demo',
    how: 'See how it works',
    paths: 'Secondary exploration',
    problem: 'The Problem',
    problemHint: 'Understand the global physical trust gap.',
    future: 'The Future',
    futureHint: 'See what verifiable physical history can enable.',
    sample: 'Example record',
    sampleTag: 'Tag',
    sampleAnimal: 'Animal',
    sampleHolder: 'Custodian',
    sampleState: 'Status',
    sampleStateValue: 'linked · finalized',
    sampleHolderValue: 'Fazenda Boa Vista',
    flowTitle: 'Follow one capture from the ear tag to verification',
    flowIntro:
      'Pick a scenario and run it. Each step shows who acts, what goes in, what comes out and what stops a forged record.',
    rolesTitle: 'Where each participant comes in',
    rolesIntro: 'The same record serves everyone along the chain, each with a different job.',
    roles: {
      field: {
        who: 'Producer, carrier, slaughterhouse',
        what: 'Register properties, animals, trips and lots in the workspace for your role.',
      },
      operator: {
        who: 'Field operator',
        what: 'Register the animal, bind the tag with Station evidence and hand over custody.',
        cta: 'Open the console',
      },
      buyer: {
        who: 'Buyer, inspector, importer',
        what: 'Check an animal by its ID without trusting the seller or this website.',
        cta: 'Verify',
        placeholder: 'Asset ID (64 hex characters)',
      },
      journey: {
        who: 'Anyone',
        what: 'See a full lot journey, from the farm to the market shelf, stage by stage.',
        cta: 'View the journey',
      },
    },
    signIn: 'Sign in',
    continueAs: 'Go to my workspace',
  },
  pt: {
    kicker: 'Rastreabilidade bovina',
    title: 'Identidade e custódia verificáveis para o rebanho.',
    support: ['O identificador físico pode mudar.', 'A identidade e a custódia verificáveis, não.'],
    demo: 'Abrir Demo',
    how: 'Ver como funciona',
    paths: 'Explorar mais',
    problem: 'O Problema',
    problemHint: 'Entenda a lacuna de confiança nas cadeias físicas.',
    future: 'O Futuro',
    futureHint: 'Veja o que um histórico físico verificável permite.',
    sample: 'Registro de exemplo',
    sampleTag: 'Brinco',
    sampleAnimal: 'Animal',
    sampleHolder: 'Custodiante',
    sampleState: 'Situação',
    sampleStateValue: 'vinculado · finalizado',
    sampleHolderValue: 'Fazenda Boa Vista',
    flowTitle: 'Acompanhe uma captura do brinco até a verificação',
    flowIntro:
      'Escolha um cenário e execute. Cada etapa mostra quem age, o que entra, o que sai e o que impede um registro falso.',
    rolesTitle: 'Onde cada participante entra',
    rolesIntro: 'O mesmo registro serve a toda a cadeia, cada um com uma tarefa diferente.',
    roles: {
      field: {
        who: 'Produtor, transportadora, frigorífico',
        what: 'Cadastre propriedades, animais, viagens e lotes no painel do seu perfil.',
      },
      operator: {
        who: 'Operador de campo',
        what: 'Registre o animal, vincule o brinco com a evidência da estação e transfira a custódia.',
        cta: 'Abrir o console',
      },
      buyer: {
        who: 'Comprador, fiscal, importador',
        what: 'Confira um animal pelo ID sem precisar confiar no vendedor nem neste site.',
        cta: 'Verificar',
        placeholder: 'ID do animal (64 caracteres hex)',
      },
      journey: {
        who: 'Qualquer pessoa',
        what: 'Veja a jornada completa de um lote, da fazenda à gôndola, etapa por etapa.',
        cta: 'Ver a jornada',
      },
    },
    signIn: 'Entrar',
    continueAs: 'Ir para o meu painel',
  },
})

const validAsset = computed(() => /^[0-9a-f]{64}$/i.test(assetId.value.trim()))

function verify(): void {
  const id = assetId.value.trim().toLowerCase()
  void router.push(validAsset.value ? `/verify/${id}` : '/verify')
}
</script>

<template>
  <main class="home">
    <section class="hero" aria-labelledby="landing-title">
      <div class="hero__text">
        <p class="hero__kicker">{{ copy.kicker }}</p>
        <h1 id="landing-title">{{ copy.title }}</h1>
        <p class="hero__support">
          {{ copy.support[0] }}<br />
          {{ copy.support[1] }}
        </p>

        <div class="hero__actions">
          <RouterLink class="hero__cta" to="/chain-history">{{ copy.demo }}</RouterLink>
          <RouterLink class="hero__secondary" to="/#fluxo">{{ copy.how }}</RouterLink>
        </div>

        <nav class="hero__paths" :aria-label="copy.paths">
          <RouterLink class="hero__path" to="/problem">
            <span>{{ copy.problem }}</span>
            <small>{{ copy.problemHint }}</small>
          </RouterLink>
          <RouterLink class="hero__path" to="/future">
            <span>{{ copy.future }}</span>
            <small>{{ copy.futureHint }}</small>
          </RouterLink>
        </nav>
      </div>

      <figure class="hero__visual">
        <img src="/lastro2.png" alt="" width="1672" height="941" />
        <figcaption class="hero__record">
          <p>{{ copy.sample }}</p>
          <dl>
            <dt>{{ copy.sampleTag }}</dt>
            <dd><code>076 0000 4521 8830</code></dd>
            <dt>{{ copy.sampleAnimal }}</dt>
            <dd><code>9f3a…c41e</code></dd>
            <dt>{{ copy.sampleHolder }}</dt>
            <dd>{{ copy.sampleHolderValue }}</dd>
            <dt>{{ copy.sampleState }}</dt>
            <dd class="hero__ok">{{ copy.sampleStateValue }}</dd>
          </dl>
        </figcaption>
      </figure>
    </section>

    <section id="fluxo" class="section" aria-labelledby="flow-title">
      <header class="section__head">
        <h2 id="flow-title">{{ copy.flowTitle }}</h2>
        <p>{{ copy.flowIntro }}</p>
      </header>
      <SystemFlow />
    </section>

    <section class="section" aria-labelledby="roles-title">
      <header class="section__head">
        <h2 id="roles-title">{{ copy.rolesTitle }}</h2>
        <p>{{ copy.rolesIntro }}</p>
      </header>

      <div class="roles">
        <article class="role">
          <h3>{{ copy.roles.field.who }}</h3>
          <p>{{ copy.roles.field.what }}</p>
          <RouterLink v-if="session" class="role__link" to="/painel">{{
            copy.continueAs
          }}</RouterLink>
          <RouterLink v-else class="role__link" to="/login">{{ copy.signIn }}</RouterLink>
        </article>

        <article class="role">
          <h3>{{ copy.roles.operator.who }}</h3>
          <p>{{ copy.roles.operator.what }}</p>
          <RouterLink class="role__link" to="/demo">{{ copy.roles.operator.cta }}</RouterLink>
        </article>

        <article class="role role--wide">
          <h3>{{ copy.roles.buyer.who }}</h3>
          <p>{{ copy.roles.buyer.what }}</p>
          <form class="role__verify" @submit.prevent="verify">
            <label class="sr-only" for="home-asset">{{ copy.roles.buyer.placeholder }}</label>
            <input
              id="home-asset"
              v-model="assetId"
              type="text"
              spellcheck="false"
              autocomplete="off"
              :placeholder="copy.roles.buyer.placeholder"
            />
            <button type="submit">{{ copy.roles.buyer.cta }}</button>
          </form>
        </article>

        <article class="role">
          <h3>{{ copy.roles.journey.who }}</h3>
          <p>{{ copy.roles.journey.what }}</p>
          <RouterLink class="role__link" to="/chain-history">{{
            copy.roles.journey.cta
          }}</RouterLink>
        </article>
      </div>
    </section>
  </main>
</template>

<style scoped>
.home {
  width: min(100% - 48px, var(--page-width));
  margin: 0 auto;
  padding-bottom: 72px;
}

.hero {
  display: grid;
  grid-template-columns: minmax(0, 1.1fr) minmax(0, 0.9fr);
  align-items: center;
  gap: 56px;
  min-height: min(calc(100svh - var(--header-height)), 760px);
  padding: 40px 0 48px;
}

.hero__kicker {
  margin: 0 0 16px;
  color: var(--primary);
  font-size: 14px;
  font-weight: 600;
}

.hero h1 {
  max-width: 15ch;
  margin: 0;
  color: var(--text);
  font-size: clamp(40px, 4.4vw, 64px);
  font-weight: 700;
  line-height: 1.04;
  letter-spacing: -0.035em;
  text-wrap: balance;
}

.hero__support {
  margin: 22px 0 0;
  color: var(--muted);
  font-size: clamp(16px, 1.3vw, 19px);
  line-height: 1.6;
}

.hero__actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 12px 20px;
  margin-top: 28px;
}

.hero__cta {
  display: inline-flex;
  align-items: center;
  min-height: 46px;
  padding: 12px 22px;
  border-radius: var(--radius-md);
  background: var(--primary);
  color: var(--on-primary);
  font-size: 16px;
  font-weight: 600;
  text-decoration: none;
}

.hero__cta:hover {
  background: var(--primary-hover);
}

.hero__secondary {
  color: var(--primary);
  font-size: 15px;
  font-weight: 600;
  text-underline-offset: 4px;
}

.hero__paths {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 250px));
  gap: 16px 28px;
  margin-top: 36px;
}

.hero__path {
  display: grid;
  gap: 4px;
  padding-top: 12px;
  border-top: 1px solid var(--border-strong);
  text-decoration: none;
}

.hero__path span {
  color: var(--text);
  font-size: 15px;
  font-weight: 600;
}

.hero__path small {
  color: var(--muted);
  font-size: 13.5px;
  line-height: 1.45;
}

.hero__path:hover {
  border-top-color: var(--accent);
}

.hero__path:hover span {
  color: var(--accent);
}

.hero__visual {
  position: relative;
  margin: 0;
}

.hero__visual img {
  display: block;
  width: 100%;
  height: auto;
  max-height: 600px;
  aspect-ratio: 1;
  object-fit: cover;
  object-position: 74% 50%;
  border-radius: var(--radius-lg);
}

.hero__record {
  position: absolute;
  bottom: 20px;
  left: -28px;
  width: min(300px, 80%);
  padding: 14px 16px;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: var(--surface);
  box-shadow: var(--shadow-md);
}

.hero__record p {
  margin: 0 0 10px;
  color: var(--muted);
  font-size: 12.5px;
  font-weight: 600;
}

.hero__record dl {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr);
  gap: 6px 14px;
  margin: 0;
  font-size: 13px;
}

.hero__record dt {
  color: var(--muted);
}

.hero__record dd {
  margin: 0;
}

.hero__record code {
  font-size: 12.5px;
}

.hero__ok {
  color: var(--success);
  font-weight: 600;
}

.section {
  padding-top: 56px;
}

.section__head {
  max-width: 760px;
  margin-bottom: 24px;
}

.section__head h2 {
  margin: 0;
  font-size: clamp(24px, 2.3vw, 32px);
  line-height: 1.15;
  letter-spacing: -0.02em;
}

.section__head p {
  margin: 10px 0 0;
  color: var(--muted);
  font-size: 16px;
  line-height: 1.55;
}

.roles {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 16px;
}

.role {
  display: grid;
  align-content: start;
  gap: 8px;
  padding: 20px;
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  background: var(--surface);
}

.role h3 {
  margin: 0;
  font-size: 16px;
}

.role p {
  margin: 0;
  color: var(--muted);
  font-size: 14.5px;
  line-height: 1.5;
}

.role__link {
  justify-self: start;
  margin-top: 6px;
  color: var(--primary);
  font-weight: 600;
  text-underline-offset: 4px;
}

.role__verify {
  display: flex;
  gap: 8px;
  margin-top: 6px;
}

.role__verify input {
  flex: 1;
  min-width: 0;
  font-family: var(--font-mono);
  font-size: 13px;
}

.role__verify button {
  padding: 0 16px;
  border: 0;
  border-radius: var(--radius-sm);
  background: var(--primary);
  color: var(--on-primary);
  font-weight: 600;
  cursor: pointer;
}

.role__verify button:hover {
  background: var(--primary-hover);
}

@media (max-width: 1100px) {
  .hero {
    gap: 40px;
  }

  .hero__record {
    left: 16px;
  }
}

@media (max-width: 860px) {
  .home {
    width: min(100% - 32px, var(--page-width));
  }

  .hero {
    grid-template-columns: minmax(0, 1fr);
    min-height: 0;
    padding-top: 28px;
  }

  .hero__visual img {
    aspect-ratio: 4 / 3;
  }

  .roles {
    grid-template-columns: minmax(0, 1fr);
  }
}

@media (max-width: 480px) {
  .hero__paths {
    grid-template-columns: minmax(0, 1fr);
  }

  .role__verify {
    flex-direction: column;
  }

  .role__verify button {
    min-height: 42px;
  }
}
</style>
