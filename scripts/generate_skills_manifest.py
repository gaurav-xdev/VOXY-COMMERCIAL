import os
import glob
import re
import json

skills = []

def extract_metadata(filepath):
    with open(filepath, 'r', encoding='utf-8') as f:
        content = f.read()
    fm_match = re.search(r'^---\s*\n(.*?)\n---', content, re.DOTALL)
    fm = fm_match.group(1) if fm_match else ''
    name_m = re.search(r'^name:\s*(.+)$', fm, re.MULTILINE)
    name = name_m.group(1).strip() if name_m else ''
    desc_m = re.search(r'description:\s*(?:>-\s*|>\s*)?\n?(.*?)(?:\n[a-z0-9_-]+:|\Z)', fm, re.DOTALL)
    desc = desc_m.group(1).strip() if desc_m else ''
    desc = re.sub(r'\s+', ' ', desc)
    return name, desc, content

# 1. SOTA Skills
sota_dirs = sorted(glob.glob('crates/skills/catalog/manifests/sota/*'))
for sdir in sota_dirs:
    if not os.path.isdir(sdir):
        continue
    sfile = os.path.join(sdir, 'SKILL.md')
    if not os.path.exists(sfile):
        continue
    sname = os.path.basename(sdir)
    name, desc, content = extract_metadata(sfile)
    display_title = sname.replace('-', ' ').title()
    if not name:
        name = sname
    
    # Categorization
    cat = 'general_engineering'
    langs = []
    checks = []
    roles = ['implementer']
    prio = 80
    
    if any(k in sname for k in ['rust', 'python', 'javascript', 'golang', 'c-cpp', 'dotnet', 'jvm', 'php', 'ruby', 'swift', 'shell']):
        cat = 'language_engineering'
        prio = 80
        roles = ['implementer', 'reviewer']
        checks = ['compilation_and_lint_check', 'idiomatic_patterns_verified']
        if 'rust' in sname:
            langs = ['rust']
            checks.extend(['memory_or_type_safety_audit', 'unsafe_code_boundary_check'])
        elif 'python' in sname:
            langs = ['python']
            checks.append('type_annotations_and_lint_check')
        elif 'javascript' in sname:
            langs = ['javascript', 'typescript']
            checks.append('typescript_strict_check')
        elif 'c-cpp' in sname:
            langs = ['c', 'cpp']
            checks.append('memory_bounds_check')
        elif 'golang' in sname:
            langs = ['go']
            checks.append('goroutine_leak_check')
    elif any(k in sname for k in ['security', 'threat', 'secrets', 'sandboxing', 'confidential', 'detection', 'devsecops']):
        cat = 'security_hardening'
        prio = 85
        roles = ['security', 'reviewer']
        checks = ['no_hardcoded_secrets', 'least_privilege_enforced', 'input_validation_and_sanitization']
    elif any(k in sname for k in ['architecture', 'api-design', 'cloud', 'database', 'kubernetes', 'llm', 'ml']):
        cat = 'architecture_design'
        prio = 80
        roles = ['architect', 'implementer']
        checks = ['architectural_boundaries_respected', 'design_tradeoffs_documented']
    elif any(k in sname for k in ['testing', 'performance', 'observability', 'async-concurrency']):
        cat = 'quality_testing'
        prio = 80
        roles = ['qa', 'implementer', 'reviewer']
        checks = ['performance_or_regression_verified', 'structured_concurrency_verified']
    elif any(k in sname for k in ['frontend', 'web-frameworks', 'cli-ux']):
        cat = 'frontend_ui'
        prio = 75
        roles = ['implementer', 'reviewer']
        checks = ['ui_accessibility_verified', 'responsive_layout_verified']
        langs = ['typescript', 'javascript', 'html', 'css']

    # Trigger tags extraction
    tags = [sname.replace('sota-', '')]
    if 'triggers:' in content.lower():
        trig_match = re.search(r'triggers:\s*([^\n\.]+)', content, re.IGNORECASE)
        if trig_match:
            raw_tags = [t.strip().lower() for t in trig_match.group(1).split(',')]
            tags.extend([t for t in raw_tags if 1 < len(t) < 30])
    
    for part in sname.split('-'):
        if part not in tags and len(part) > 2:
            tags.append(part)

    skills.append({
        'id': f'sota:{sname}',
        'name': f'SOTA {display_title}',
        'category': cat,
        'source_repo': 'https://github.com/martinholovsky/SOTA-skills.git',
        'version_pinned': '2e9b417b14b21e62b6a35ceec803cf5d1cd081ef',
        'author': 'Martin Holovsky',
        'license': 'CC-BY-4.0 / Apache-2.0',
        'description': desc or f'State-of-the-art engineering rules for {sname}',
        'trigger_tags': sorted(list(set(tags))),
        'target_roles': roles,
        'languages': langs,
        'required_permissions': ['read_files'],
        'network_required': False,
        'risk_level': 'read',
        'doc_path': f'manifests/sota/{sname}/SKILL.md',
        'required_checks': checks,
        'conflict_priority': prio,
        'enabled': True
    })

# 2. Addy Osmani Skills
addy_dirs = sorted(glob.glob('crates/skills/catalog/manifests/addy/*'))
for adir in addy_dirs:
    if not os.path.isdir(adir):
        continue
    sfile = os.path.join(adir, 'SKILL.md')
    if not os.path.exists(sfile):
        continue
    sname = os.path.basename(adir)
    name, desc, content = extract_metadata(sfile)
    display_title = sname.replace('-', ' ').title()
    if not name:
        name = sname
    
    cat = 'general_engineering'
    roles = ['implementer']
    prio = 70
    checks = []
    langs = []
    
    if sname in ['test-driven-development', 'debugging-and-error-recovery', 'browser-testing-with-devtools', 'performance-optimization', 'observability-and-instrumentation', 'code-simplification', 'code-review-and-quality']:
        cat = 'quality_testing'
        prio = 75
        roles = ['qa', 'reviewer', 'implementer']
        if 'test-driven' in sname:
            checks = ['failing_test_written_first', 'all_unit_and_regression_tests_pass']
        elif 'debug' in sname:
            checks = ['root_cause_identified', 'regression_test_added']
        elif 'review' in sname:
            checks = ['independent_diff_audit_passed', 'zero_regressions_verified']
        elif 'simplification' in sname:
            checks = ['dead_code_eliminated', 'no_behavioral_regression']
    elif sname in ['security-and-hardening']:
        cat = 'security_hardening'
        prio = 85
        roles = ['security', 'reviewer']
        checks = ['no_hardcoded_secrets', 'least_privilege_enforced', 'input_validation_and_sanitization']
    elif sname in ['api-and-interface-design', 'planning-and-task-breakdown', 'documentation-and-adrs', 'deprecation-and-migration']:
        cat = 'architecture_design'
        prio = 75
        roles = ['architect', 'implementer']
        checks = ['architectural_boundaries_respected', 'design_tradeoffs_documented']
    elif sname in ['git-workflow-and-versioning', 'constraint-driven-development', 'doubt-driven-development', 'incremental-implementation', 'source-driven-development', 'spec-driven-development', 'shipping-and-launch']:
        cat = 'open_source_practices'
        prio = 80
        roles = ['implementer', 'reviewer']
        checks = ['local_conventions_followed', 'minimal_diff_verified']
    elif sname in ['frontend-ui-engineering']:
        cat = 'frontend_ui'
        prio = 75
        roles = ['implementer', 'reviewer']
        langs = ['typescript', 'javascript', 'html', 'css']
        checks = ['ui_accessibility_verified', 'responsive_layout_verified']

    tags = [sname] + sname.split('-')
    if 'tdd' in sname or 'test-driven' in sname:
        tags.extend(['tdd', 'test', 'unit test', 'red-green-refactor', 'regression'])
    if 'debug' in sname:
        tags.extend(['debug', 'bug', 'panic', 'crash', 'stacktrace', 'fix', 'investigation', 'root-cause'])
    if 'review' in sname:
        tags.extend(['review', 'audit', 'pr', 'code review', 'diff'])
    if 'security' in sname:
        tags.extend(['security', 'vulnerability', 'threat', 'hardening', 'cve'])
    if 'simplify' in sname:
        tags.extend(['simplify', 'refactor', 'clean', 'dead code'])

    skills.append({
        'id': f'addy:{sname}',
        'name': f'Addy {display_title}',
        'category': cat,
        'source_repo': 'https://github.com/addyosmani/agent-skills.git',
        'version_pinned': 'aeac41d4d005999432bb7312f7ff06383a91d59d',
        'author': 'Addy Osmani',
        'license': 'MIT',
        'description': desc or f'Engineering best practices for {sname}',
        'trigger_tags': sorted(list(set([t.lower() for t in tags if len(t) > 1]))),
        'target_roles': roles,
        'languages': langs,
        'required_permissions': ['read_files'],
        'network_required': False,
        'risk_level': 'read',
        'doc_path': f'manifests/addy/{sname}/SKILL.md',
        'required_checks': checks,
        'conflict_priority': prio,
        'enabled': True
    })

# 3. Mohit Senior Open Source Engineer Skill
mohit_file = 'crates/skills/catalog/manifests/mohit/senior-open-source-engineer/SKILL.md'
if os.path.exists(mohit_file):
    name, desc, content = extract_metadata(mohit_file)
    skills.append({
        'id': 'oss:senior-open-source-engineer',
        'name': 'Senior Open Source Engineer',
        'category': 'open_source_practices',
        'source_repo': 'https://github.com/mohitdebian/Senior-Open-Source-Engineer-Skill.git',
        'version_pinned': '2cd5fdb18de8468dc2fd145999b41dbdea844c63',
        'author': 'Mohit Kumar',
        'license': 'MIT',
        'description': desc or 'Behave like a senior software engineer contributing to a mature open-source repository.',
        'trigger_tags': ['open source', 'oss', 'contribute', 'maintainer', 'minimal diff', 'repo adaptation', 'pr', 'pull request', 'issue', 'senior dev', 'architecture learning', 'native style', 'ponytail'],
        'target_roles': ['architect', 'implementer', 'qa', 'reviewer'],
        'languages': [],
        'required_permissions': ['read_files'],
        'network_required': False,
        'risk_level': 'read',
        'doc_path': 'manifests/mohit/senior-open-source-engineer/SKILL.md',
        'required_checks': ['repository_conventions_inspected', 'minimal_diff_verified', 'native_test_style_followed', 'maintainer_friendly_pr_format'],
        'conflict_priority': 90,
        'enabled': True
    })

# 4. Built-in system capabilities (preserved from v1)
builtins = [
    {
        'id': 'frontend-ui:pmndrs-uikit',
        'name': 'pmndrs / uikit Spatial 3D UI',
        'category': 'frontend_ui',
        'source_repo': 'https://github.com/pmndrs/uikit.git',
        'version_pinned': '5a81dbf7b11d35aeaebeba5d9284ba5f22f518e1',
        'author': 'pmndrs',
        'license': 'MIT',
        'description': '3D Spatial user interface components and layouts with Three.js / React Three Fiber.',
        'trigger_tags': ['3d', 'webgl', 'three.js', 'r3f', 'uikit', 'spatial', 'spatial-ui', 'threejs'],
        'target_roles': ['implementer'],
        'languages': ['typescript', 'javascript'],
        'required_permissions': ['read_files'],
        'network_required': False,
        'risk_level': 'read',
        'doc_path': 'manifests/pmndrs_uikit.md',
        'required_checks': ['ui_accessibility_verified'],
        'conflict_priority': 70,
        'enabled': True
    },
    {
        'id': 'workflow:temporal-orchestration',
        'name': 'Temporal Durable Workflow Engine',
        'category': 'workflow_orchestration',
        'source_repo': 'https://github.com/temporalio/temporal.git',
        'version_pinned': '994ba6f5f92275f0a0d9fcfbe55928f090d81005',
        'author': 'Temporal Technologies',
        'license': 'MIT',
        'description': 'Durable, fault-tolerant stateful workflows and activity compensations.',
        'trigger_tags': ['workflow', 'temporal', 'orchestration', 'durable', 'compensation', 'saga', 'async-workflow', 'retry-policy'],
        'target_roles': ['architect', 'implementer'],
        'languages': [],
        'required_permissions': ['read_files'],
        'network_required': False,
        'risk_level': 'read',
        'doc_path': 'manifests/temporal_workflows.md',
        'required_checks': ['architectural_boundaries_respected'],
        'conflict_priority': 75,
        'enabled': True
    },
    {
        'id': 'coding-tools:mcp-suite',
        'name': 'Coding Tools MCP Runtime Contract',
        'category': 'coding_tools',
        'source_repo': 'https://github.com/xyTom/coding-tools-mcp.git',
        'version_pinned': 'd7c2dda48bcedbd066c7dbc24a1b63205384d269',
        'author': 'xyTom',
        'license': 'MIT',
        'description': 'Standardized coding primitives over MCP contract: patch application, AST search, diff verification, and process isolation.',
        'trigger_tags': ['coding', 'patch', 'ast', 'mcp', 'git-diff', 'code-harness', 'ripgrep', 'patch-engine'],
        'target_roles': ['implementer'],
        'languages': [],
        'required_permissions': ['read_files', 'write_files'],
        'network_required': False,
        'risk_level': 'modify',
        'doc_path': 'manifests/coding_tools_mcp_spec.md',
        'required_checks': ['compilation_and_lint_check'],
        'conflict_priority': 75,
        'enabled': True
    },
    {
        'id': 'web-research:firecrawl-extraction',
        'name': 'Firecrawl Web Research and Extraction',
        'category': 'web_research',
        'source_repo': 'https://github.com/firecrawl/firecrawl.git',
        'version_pinned': '7cca3edf968ebb40593d0fb92064d241bc89f8e3',
        'author': 'Firecrawl',
        'license': 'Apache-2.0',
        'description': 'Intelligent web scraping, clean markdown extraction, sitemap crawling, and structured search data capture.',
        'trigger_tags': ['web-research', 'scrape', 'firecrawl', 'crawl', 'extract-markdown', 'url-fetch', 'web-search', 'documentation-crawl'],
        'target_roles': ['implementer'],
        'languages': [],
        'required_permissions': ['read_files'],
        'network_required': True,
        'risk_level': 'privileged',
        'doc_path': 'manifests/firecrawl_firecrawl-build-scrape.md',
        'required_checks': [],
        'conflict_priority': 70,
        'enabled': True
    }
]
skills.extend(builtins)


manifest = {
    'version': '2.0.0',
    'last_updated': '2026-10-24T16:00:00Z',
    'total_capabilities': len(skills),
    'capabilities': skills
}

output_path = 'crates/skills/catalog/skills_manifest.json'
with open(output_path, 'w', encoding='utf-8') as f:
    json.dump(manifest, f, indent=2)

print(f'Successfully generated {output_path} with {len(skills)} capabilities!')
