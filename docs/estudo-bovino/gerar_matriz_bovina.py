from openpyxl import Workbook
from openpyxl.styles import Font, PatternFill, Alignment, Border, Side
from openpyxl.utils import get_column_letter
from openpyxl.worksheet.table import Table, TableStyleInfo
from openpyxl.formatting.rule import ColorScaleRule
from openpyxl.chart import BarChart, Reference
from openpyxl import load_workbook

OUT = '/mnt/2b0f21e8-60f9-490c-bfbe-b6f4606ac236/lastro/docs/estudo-bovino/MATRIZ_RASTREABILIDADE_BOVINA.xlsx'

THEME = {
    'primary': '1F4E79',
    'light': 'D6E3F0',
    'accent': '0D47A1',
    'green': 'E8F5E9',
    'orange': 'FFF3E0',
    'red': 'FFEBEE',
    'purple': 'F3E5F5',
    'gray': 'F4F6F8',
}
SERIF = 'Georgia'
SANS = 'Calibri'
thin = Side(style='thin', color='D1D5DB')
medium = Side(style='medium', color=THEME['primary'])

wb = Workbook()
ws = wb.active
ws.title = 'Overview'


def setup(ws):
    ws.sheet_view.showGridLines = False
    ws.column_dimensions['A'].width = 3
    ws.freeze_panes = 'B6'


def title(ws, text, subtitle, last_col=8):
    ws.merge_cells(start_row=2, start_column=2, end_row=2, end_column=last_col)
    ws['B2'] = text
    ws['B2'].font = Font(name=SERIF, size=20, bold=True, color=THEME['primary'])
    ws['B2'].alignment = Alignment(vertical='center')
    ws.row_dimensions[2].height = 35
    ws.merge_cells(start_row=3, start_column=2, end_row=3, end_column=last_col)
    ws['B3'] = subtitle
    ws['B3'].font = Font(name=SANS, size=10, italic=True, color='666666')
    ws['B3'].alignment = Alignment(wrap_text=True, vertical='center')
    ws.row_dimensions[3].height = 30


def section(ws, row, text, last_col=8):
    ws.merge_cells(start_row=row, start_column=2, end_row=row, end_column=last_col)
    c = ws.cell(row, 2, text)
    c.font = Font(name=SERIF, size=13, bold=True, color=THEME['primary'])
    c.fill = PatternFill('solid', fgColor=THEME['light'])
    c.alignment = Alignment(vertical='center')
    ws.row_dimensions[row].height = 25


def table(ws, start_row, start_col, headers, rows, widths=None, name='Table1'):
    for col, header in enumerate(headers, start=start_col):
        c = ws.cell(start_row, col, header)
        c.font = Font(name=SERIF, size=10, bold=True, color='FFFFFF')
        c.fill = PatternFill('solid', fgColor=THEME['primary'])
        c.alignment = Alignment(horizontal='center', vertical='center', wrap_text=True)
        c.border = Border(top=medium, bottom=medium)
    ws.row_dimensions[start_row].height = 32
    for r_idx, row in enumerate(rows, start=start_row + 1):
        for c_idx, value in enumerate(row, start=start_col):
            c = ws.cell(r_idx, c_idx, value)
            c.font = Font(name=SANS, size=10)
            c.alignment = Alignment(vertical='top', wrap_text=True, horizontal='left')
            c.border = Border(bottom=thin)
            if r_idx % 2 == 0:
                c.fill = PatternFill('solid', fgColor='FAFCFE')
        ws.row_dimensions[r_idx].height = 42 if len(row) >= 5 else 25
    end_row = start_row + len(rows)
    end_col = start_col + len(headers) - 1
    ref = f'{get_column_letter(start_col)}{start_row}:{get_column_letter(end_col)}{end_row}'
    tab = Table(displayName=name, ref=ref)
    tab.tableStyleInfo = TableStyleInfo(name='TableStyleMedium2', showFirstColumn=False, showLastColumn=False, showRowStripes=True, showColumnStripes=False)
    ws.add_table(tab)
    if widths:
        for idx, width in enumerate(widths, start=start_col):
            ws.column_dimensions[get_column_letter(idx)].width = width
    return start_row, end_row, start_col, end_col

# Overview
setup(ws)
title(ws, 'Matriz de rastreabilidade bovina — Lastro', 'Proposta técnica para animal individual, lotes, peso, localização, status, documentos e custódia. Gerado em 23/09/2026.', 9)
section(ws, 5, 'LEITURA EXECUTIVA', 9)
insights = [
    'Decisão de domínio: o AnimalID individual permanece canônico; lote é manifesto versionado com raiz criptográfica.',
    'Peso, localização, status, trânsito e venda são eventos com proveniência, não campos livres editáveis.',
    'O smart contract ancora integridade, sequência, compromissos e custódia; não substitui GTA/e-GTA, SISBOV/PNIB, NF-e ou prova jurídica de propriedade.',
    'Consulta pública deve mostrar prova mínima e localização generalizada; documentos e identidade ficam em acesso autenticado.',
    'Prioridades: intent/nonce on-chain, reconciliação server-side, registry de Stations, adapters oficiais e privacidade por desenho.',
]
for i, text in enumerate(insights, start=6):
    ws.merge_cells(start_row=i, start_column=2, end_row=i, end_column=9)
    ws.cell(i, 2, '• ' + text)
    ws.cell(i, 2).font = Font(name=SANS, size=11)
    ws.cell(i, 2).alignment = Alignment(wrap_text=True, vertical='center')
    ws.row_dimensions[i].height = 30
section(ws, 13, 'NAVEGAÇÃO', 9)
links = [('Data Model', 'Entidades e fronteiras de autoridade'), ('Event Catalog', 'Eventos de negócio e transições'), ('Requirements', 'Requisitos priorizados'), ('Sources', 'Fontes oficiais e limites')]
for i, (sheet, desc) in enumerate(links, start=14):
    c = ws.cell(i, 2, sheet)
    c.hyperlink = f"#'{sheet}'!A1"
    c.font = Font(name=SANS, size=11, color=THEME['accent'], underline='single')
    ws.cell(i, 3, desc).font = Font(name=SANS, size=10, color='555555')
section(ws, 20, 'INDICADORES DA MATRIZ', 9)
ws['B21'] = 'Requisitos P0/P1/P2'
ws['C21'] = '=COUNTIF(Requirements!C:C,"P0")+COUNTIF(Requirements!C:C,"P1")+COUNTIF(Requirements!C:C,"P2")'
ws['B22'] = 'Requisitos on-chain'
ws['C22'] = '=COUNTIF(Requirements!F:F,"On-chain")'
ws['B23'] = 'Eventos catalogados'
ws['C23'] = '=COUNTA(\'Event Catalog\'!A:A)-1'
for r in (21, 22, 23):
    ws.cell(r, 2).font = Font(name=SANS, bold=True)
    ws.cell(r, 3).font = Font(name=SANS, bold=True, color=THEME['accent'])
    ws.cell(r, 3).number_format = '#,##0'
    ws.cell(r, 2).border = Border(bottom=thin)
    ws.cell(r, 3).border = Border(bottom=thin)
ws['B26'] = 'Nota de escopo:'
ws['B26'].font = Font(name=SANS, bold=True, color='7A1F1F')
ws.merge_cells('C26:I26')
ws['C26'] = 'Esta matriz é proposta de arquitetura. Ela não é certificação SISBOV/PNIB, autorização de GTA, parecer jurídico ou garantia de propriedade.'
ws['C26'].font = Font(name=SANS, italic=True, color='7A1F1F')
ws['C26'].alignment = Alignment(wrap_text=True)
ws.row_dimensions[26].height = 32
for col, width in {'B':28,'C':26,'D':18,'E':18,'F':18,'G':18,'H':18,'I':18}.items():
    ws.column_dimensions[col].width = width

# Data Model
ws = wb.create_sheet('Data Model')
setup(ws)
title(ws, 'Modelo de dados e autoridade', 'Separação entre identidade, lote, documento, custódia, propriedade alegada e fonte oficial.', 9)
section(ws, 5, 'ENTIDADES', 9)
entities = [
    ['Animal', 'Solana + PostgreSQL', 'Identidade individual e estado terminal', 'Não é prova automática de propriedade', 'AnimalID, namespace, estado, sequência, último hash'],
    ['AnimalIdentifier', 'Protocolo + fonte', 'Vincula RFID/identificador oficial ao animal', 'Hash não equivale a número oficial', 'namespace, valor protegido, status, origem'],
    ['LotManifest', 'PostgreSQL + hash Solana', 'Agrupa animais/quantidades por finalidade', 'Não substitui estado individual', 'lot_id, versão, raiz, membros/quantidade, GTA'],
    ['WeightObservation', 'Station/balança + evidência', 'Mede peso individual ou de lote', 'Assinatura não prova calibração por si só', 'gramas, instrumento, método, hora, incerteza'],
    ['LocationObservation', 'Dispositivo/estabelecimento/fonte', 'Registra local com precisão controlada', 'GPS assinado não prova presença absoluta', 'commitment, região, precisão, fonte, hora'],
    ['Movement', 'Documentos + eventos', 'Representa trânsito e chegada', 'Não emitir GTA por conta própria', 'origem, destino, finalidade, veículo, GTA, status'],
    ['OfficialDocumentReference', 'Fonte oficial/documento', 'Conecta evento a GTA, NF-e, inspeção', 'Documento privado não vira ato oficial', 'tipo, emissor, série, número, hash, verificação'],
    ['TransferIntent', 'API + nonce Solana', 'Proposta e aceite de custódia/venda', 'Custódia não é propriedade civil', 'remetente, recebedor, aceite, prazo, documentos'],
    ['Party/RoleAssignment', 'Diretório protegido', 'Pessoas, organizações e papéis', 'Custodiante não deve ser exibido como proprietário', 'party_id, papel, wallet, escopo, validade'],
    ['EvidencePackage', 'PostgreSQL/object storage', 'Prova em camadas de exposição', 'Não retornar veredicto pronto do backend', 'manifesto, hashes, anexos, origem, cadeia de acesso'],
    ['Dispute', 'PostgreSQL + evento', 'Contestação e resolução', 'Não apagar histórico contestado', 'evento, razão, parte, estado, decisão, autoridade'],
]
table(ws, 6, 2, ['Entidade','Fonte canônica','Função','Limite importante','Campos conceituais'], entities, [25,24,34,42,46], 'EntitiesTable')
section(ws, 21, 'NÍVEIS DE PROVENIÊNCIA', 9)
prov = [
    ['OBSERVED_BY_STATION', 'Station/firmware', 'Dispositivo assinou observação física', 'Não prova fato físico correto'],
    ['DECLARED_BY_OPERATOR', 'Usuário/organização', 'Parte declarou o dado', 'Exige identidade e papel'],
    ['DOCUMENT_ATTACHED', 'Operador/documento', 'Documento foi anexado', 'Ainda não confirmado pela fonte'],
    ['DOCUMENT_VERIFIED', 'Parser/assinatura/fonte', 'Formato ou assinatura verificados', 'Não substitui autoridade'],
    ['CONFIRMED_BY_OFFICIAL_SOURCE', 'MAPA/OESA/SEFAZ/SIF etc.', 'Fonte oficial confirmou', 'Requer integração autorizada'],
    ['DISPUTED', 'Mecanismo de disputa', 'Há conflito ou contestação', 'Bloquear novas transições conforme política'],
    ['REVOKED', 'Registry/autoridade', 'Credencial ou documento invalidado', 'Preservar histórico e motivo'],
]
table(ws, 22, 2, ['Nível','Fonte típica','O que significa','O que não significa'], prov, [34,28,48,42], 'ProvenanceTable')

# Event catalog
ws = wb.create_sheet('Event Catalog')
setup(ws)
title(ws, 'Catálogo de eventos bovinos', 'Eventos append-only. O estado atual é projeção derivada, nunca edição livre.', 10)
section(ws, 5, 'EVENTOS E TRANSIÇÕES', 10)
events = [
    ['ANIMAL_REGISTERED','Animal','novo','Animal ainda não registrado','REGISTERED','produtor + fonte/documento','identificador, origem, documento, schema'],
    ['IDENTIFIER_ATTACHED','Animal','ativo','REGISTERED','ACTIVE','Station + operador','namespace, identificador, device, hora'],
    ['IDENTIFIER_REPLACED','Animal','reidentificação','ACTIVE','ACTIVE','Station + custodiante + documento','antigo, novo, motivo, revisão'],
    ['WEIGHT_OBSERVED','Animal/Lote','medição','qualquer não terminal','mesmo','balança/Station + operador','gramas, unidade, método, instrumento'],
    ['LOCATION_OBSERVED','Animal/Lote','local','qualquer não terminal','mesmo','dispositivo/estabelecimento','commitment, região, precisão, fonte'],
    ['MOVEMENT_CREATED','Lote/Animal','movimento','ACTIVE/CLEARED','MOVEMENT_PENDING','remetente + documento','origem, destino, finalidade, GTA, quantidade'],
    ['MOVEMENT_STARTED','Lote','transporte','MOVEMENT_PENDING','IN_TRANSIT','remetente + transportador','veículo, lacre, hora, GTA'],
    ['ARRIVAL_CONFIRMED','Lote/Animal','chegada','IN_TRANSIT','RECEIVED','destino/fonte oficial','quantidade recebida, hora, divergência'],
    ['QUARANTINE_ENTERED','Lote/Animal','sanitário','IN_TRANSIT/RECEIVED','IN_QUARANTINE','fonte sanitária/papel autorizado','estabelecimento, autorização, data'],
    ['QUARANTINE_RELEASED','Lote/Animal','liberação','IN_QUARANTINE','CLEARED','fonte sanitária','documento, data, condições'],
    ['TRANSFER_PROPOSED','Animal/Lote','custódia/venda','ACTIVE/RECEIVED','SOLD_PENDING_ACCEPTANCE','custodiante atual','recebedor, documentos, prazo'],
    ['TRANSFER_ACCEPTED','Animal/Lote','aceite','SOLD_PENDING_ACCEPTANCE','ACCEPTED','comprador/recebedor','intent, wallet, hora, escopo'],
    ['CUSTODY_TRANSFERRED','Animal/Lote','finalização','ACCEPTED','CUSTODY_TRANSFERRED','programa Solana + nonce','predecessor, documentos, destino'],
    ['SLAUGHTER_RECEIVED','Lote','abate','RECEIVED/CLEARED','RECEIVED_FOR_SLAUGHTER','abatedouro/fonte','GTA, inspeção, quantidade'],
    ['SLAUGHTER_CONFIRMED','Animal/Lote','abate','RECEIVED_FOR_SLAUGHTER','SLAUGHTERED','estabelecimento/fonte','data, lote produto, evidência'],
    ['ANIMAL_DEAD','Animal','morte','ACTIVE/RECEIVED/QUARANTINE','DEAD','papel autorizado','causa/categoria, data, documento'],
    ['DISPUTE_OPENED','Animal/Lote/Evento','contestação','qualquer não terminal','DISPUTED','parte autorizada','evento, razão, escopo'],
    ['EVENT_CORRECTED','Evento','correção','qualquer','projeção corrigida','autoridade/papel autorizado','evento anterior, motivo, novo hash'],
    ['SPLIT_LOT','Lote','linhagem','ACTIVE','ACTIVE','custodiante + sistema','lote pai, filhos, raízes, quantidades'],
    ['MERGE_LOT','Lote','linhagem','ACTIVE','ACTIVE','custodiante + sistema','lotes pais, novo lote, compatibilidade'],
    ['LOT_CLOSED','Lote','encerramento','ACTIVE/RECEIVED','CLOSED','papel/fonte aplicável','motivo, documento, quantidade final'],
]
table(ws, 6, 2, ['Evento','Objeto','Categoria','Estado anterior','Estado novo','Autoridade mínima','Dados mínimos'], events, [28,18,22,27,28,35,48], 'EventsTable')
section(ws, 31, 'REGRAS DE INTEGRIDADE', 10)
rules = [
    ['1','Todo evento tem event_id idempotente e event_hash único.','Previne duplicação e replay.'],
    ['2','Todo evento aponta para previous_event_hash quando a transição exige continuidade.','Impede fork silencioso do histórico.'],
    ['3','Transferência usa nonce/version on-chain.','Impede transação antiga depois de supersessão.'],
    ['4','Correção gera evento compensatório.','Preserva histórico original e motivo da correção.'],
    ['5','Status só muda por evento permitido e papel autorizado.','Impede alteração de status por CRUD.'],
    ['6','Documento e fonte possuem status próprio.','Assinado não significa oficial.'],
]
table(ws, 32, 2, ['ID','Regra','Objetivo'], rules, [10,72,50], 'IntegrityTable')

# Requirements
ws = wb.create_sheet('Requirements')
setup(ws)
title(ws, 'Matriz de requisitos e prioridades', 'Prioridade: P0 bloqueia a ideia; P1 é necessária para piloto; P2 prepara escala e governança.', 10)
section(ws, 5, 'REQUISITOS', 10)
reqs = [
    ['R-001','P0','Identidade','Manter AnimalID individual canônico; lote não substitui animal.','On-chain','Fundação do domínio','Owner técnico','Fase 1'],
    ['R-002','P0','Proveniência','Separar observado, declarado, documentado e confirmado por fonte oficial.','API + Evidence','Evita claims indevidos','Arquitetura','Fase 1'],
    ['R-003','P0','Custódia/propriedade','Não apresentar current_custodian como proprietário legal.','Domínio + UI','Evita risco jurídico','Produto/Jurídico','Fase 1'],
    ['R-004','P0','Concorrência','Adicionar intent/nonce on-chain para invalidar transação antiga.','On-chain','Evita dupla venda','Blockchain','Fase 2'],
    ['R-005','P0','Lotes','Implementar LotManifest versionado com split/merge e raiz criptográfica.','API + On-chain','Preserva linhagem','Backend','Fase 2'],
    ['R-006','P0','Movimentação','Referenciar GTA/e-GTA, origem, destino, finalidade, veículo e quantidade.','API + Evidence','Integra rastreabilidade real','Integrações','Fase 2'],
    ['R-007','P1','Peso','Registrar peso em unidade fixa, instrumento, calibração, método e hora.','Agent + API','Medição auditável','IoT','Fase 3'],
    ['R-008','P1','Localização','Separar localização privada, referência oficial e região pública generalizada.','API + UI','Reduz risco de exposição','Security','Fase 3'],
    ['R-009','P1','Status','Modelar status como máquina de estados com autoridade por transição.','On-chain + API','Evita status livre','Domain','Fase 1'],
    ['R-010','P1','Transferência','Exigir aceite explícito do comprador/recebedor e documentos.','Wallet + API','Distingue venda de custódia','Produto','Fase 2'],
    ['R-011','P1','Reconciliador','Finalização e projeção não dependem do navegador.','API worker','Recuperação operacional','SRE','Fase 2'],
    ['R-012','P1','Documentos','Guardar referência, hash, emissor, validade, parser e resposta da fonte.','PostgreSQL','Cadeia de custódia','Backend','Fase 2'],
    ['R-013','P1','Privacidade','Publicar somente prova mínima; PII/documentos fora da cadeia pública.','All','LGPD/privacy by design','DPO/Security','Fase 1'],
    ['R-014','P1','Acesso','RBAC/ABAC, tenant isolation, escopos e consulta pública não enumerável.','API + UI','Internet com segurança','Security','Fase 5'],
    ['R-015','P1','Station','Registry, provisionamento, rotação, revogação e firmware conhecido.','Chain + Firmware','Root of trust','Hardware','Fase 5'],
    ['R-016','P2','Fontes oficiais','Adapters versionados para MAPA/OESA/GTA/SEFAZ/inspeção, quando autorizados.','API','Interoperabilidade','Integrações','Fase 4'],
    ['R-017','P2','EvidencePackage','Perfis public, restricted e internal/legal com manifesto de minimização.','API + Verifier','Auditoria segura','Security','Fase 1'],
    ['R-018','P2','Pós-abate','Criar lineage de carcaça/produto após abate, se escopo incluir consumidor final.','API + Chain','Cadeia completa','Produto','Fase 6'],
    ['R-019','P2','Incidentes','Workflow de incidentes, registro, forense e comunicação conforme risco aplicável.','API + Ops','Resposta LGPD','SRE/DPO','Fase 5'],
    ['R-020','P2','Correção/disputa','Correção por evento, disputa, bloqueio e resolução sem apagar histórico.','API + Chain','Governança','Produto/Jurídico','Fase 2'],
]
table(ws, 6, 2, ['ID','Prioridade','Área','Requisito','Camada','Por que importa','Responsável','Fase'], reqs, [12,13,22,60,24,34,22,14], 'RequirementsTable')
section(ws, 29, 'CONTAGEM POR PRIORIDADE', 10)
for r, p in enumerate(['P0','P1','P2'], start=30):
    ws.cell(r, 2, p)
    ws.cell(r, 3, f'=COUNTIF(C:C,B{r})')
    ws.cell(r, 2).font = Font(name=SANS, bold=True)
    ws.cell(r, 3).number_format = '#,##0'
    ws.cell(r, 3).font = Font(name=SANS, bold=True, color=THEME['accent'])
ws.conditional_formatting.add('C30:C32', ColorScaleRule(start_type='min', start_color='E8F5E9', end_type='max', end_color='FFCDD2'))
chart = BarChart()
chart.type = 'bar'
chart.title = 'Requisitos por prioridade'
chart.y_axis.title = 'Prioridade'
chart.x_axis.title = 'Quantidade'
data = Reference(ws, min_col=3, min_row=29, max_row=32)
cats = Reference(ws, min_col=2, min_row=30, max_row=32)
chart.add_data(data, titles_from_data=True)
chart.set_categories(cats)
chart.height = 6
chart.width = 10
ws.add_chart(chart, 'E29')

# Sources
ws = wb.create_sheet('Sources')
setup(ws)
title(ws, 'Fontes oficiais e limites de interpretação', 'Fontes usadas para separar rastreabilidade, trânsito, propriedade/custódia e proteção de dados.', 8)
section(ws, 5, 'FONTES', 8)
sources = [
    ['SISBOV','MAPA','Sistema oficial de identificação individual; adesão em regra voluntária salvo obrigação/controle oficial.','https://www.gov.br/agricultura/pt-br/assuntos/sanidade-animal-e-vegetal/saude-animal/cgtqa/dpc/sisbov'],
    ['PNIB','MAPA','Plano de identificação individual, sistema integrado e implementação progressiva.','https://www.gov.br/agricultura/pt-br/assuntos/sanidade-animal-e-vegetal/saude-animal/rastreabilidade-animal/pnib'],
    ['Portaria PNIB','DOU/MAPA','Etapas, Base Central, interoperabilidade estadual e cronograma até 2033.','https://www.in.gov.br/en/web/dou/-/portaria-sda/mapa-n-1.331-de-21-de-julho-de-2025-643581903'],
    ['Lei 12.097/2009','Planalto','Rastreabilidade, GTA, nota fiscal, registros oficiais, registros privados e retenção.','https://www.planalto.gov.br/ccivil_03/_ato2007-2010/2009/lei/L12097.htm'],
    ['Decreto 7.623/2011','Planalto','Sistema público, numeração MAPA e homologação de protocolos voluntários.','https://www.planalto.gov.br/ccivil_03/_ato2011-2014/2011/decreto/d7623.htm'],
    ['Manual GTA bovinos','MAPA/WikiSDA','Origem, destino, finalidade, veículo, quantidade, trânsito, quarentena e abate.','https://wikisda.agricultura.gov.br/pt-br/Sa%C3%BAde-Animal/tr%C3%A2nsito_bovinos'],
    ['Código Civil','Câmara/Legislação','Compra e venda, posse, detenção, propriedade e tradição.','https://www2.camara.leg.br/legin/fed/lei/2002/lei-10406-10-janeiro-2002-432893-norma-pl.html'],
    ['LGPD','Planalto','Dados pessoais, princípios, bases legais, direitos, segurança e governança.','https://www.planalto.gov.br/ccivil_03/_ato2015-2018/2018/lei/l13709.htm'],
    ['Resolução ANPD 15/2024','DOU/ANPD','Comunicação de incidentes de segurança e prazos aplicáveis.','https://www.in.gov.br/en/web/dou/-/resolucao-cd/anpd-n-15-de-24-de-abril-de-2024-556243024'],
]
table(ws, 6, 2, ['Fonte','Órgão','Uso na proposta','URL'], sources, [24,22,68,55], 'SourcesTable')
for row in range(7, 7 + len(sources)):
    cell = ws.cell(row, 5)
    cell.hyperlink = ws.cell(row, 5).value
    cell.font = Font(name=SANS, size=10, color=THEME['accent'], underline='single')
section(ws, 18, 'LIMITES', 8)
limits = [
    ['A blockchain privada não substitui automaticamente SISBOV, PNIB, GTA/e-GTA, NF-e, inspeção ou cadastro OESA/SVO.','Integração, homologação ou convênio devem ser confirmados com MAPA/OESA e assessoria jurídica.'],
    ['AnimalState e current_custodian não constituem matrícula legal nem prova conclusiva de propriedade.','Separar custódia operacional, compra, entrega, posse e titularidade documentada.'],
    ['Hash de RFID, GPS e assinatura de Station não resolvem o problema do oráculo.','Registrar proveniência, equipamento, credencial, fonte, confiança e evidências complementares.'],
    ['CPF/CNPJ, coordenadas, horários, carteiras e hashes correlacionáveis podem ser dados pessoais.','Manter dados detalhados off-chain, aplicar minimização, controle de acesso e avaliação LGPD.'],
]
table(ws, 19, 2, ['Limite','Resposta de arquitetura'], limits, [72,72], 'LimitsTable')

# General styling and print settings
for sheet in wb.worksheets:
    sheet.sheet_properties.pageSetUpPr.fitToPage = True
    sheet.page_setup.fitToWidth = 1
    sheet.page_setup.fitToHeight = 0
    sheet.page_margins.left = 0.25
    sheet.page_margins.right = 0.25
    sheet.page_margins.top = 0.5
    sheet.page_margins.bottom = 0.5
    for row in sheet.iter_rows():
        for cell in row:
            if cell.value is not None and cell.alignment == Alignment():
                cell.alignment = Alignment(vertical='top', wrap_text=True)

wb.save(OUT)
print(OUT)
