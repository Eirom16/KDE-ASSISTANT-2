"""Text-only Spanish evaluation. No devices, models, downloads or application startup."""
import math
import re
import unicodedata


# Authored references are CC0. A speaker's recordings need separate consent.
CORPUS = [
    dict(id='short', reference='Hola, ¿qué hora es?'),
    dict(id='natural', reference='Explícame cómo organizar mis documentos sin mover ningún archivo.'),
    dict(id='numbers', reference='Anota veintitrés, ciento cinco y dos mil veintiséis.',
         checks=[['veintitrés', '23'], ['ciento cinco', '105'], ['dos mil veintiséis', '2026']]),
    dict(id='applications', reference='¿Cómo puedo configurar Dolphin, Firefox y LibreOffice?',
         checks=[['Dolphin'], ['Firefox'], ['LibreOffice', 'Libre Office']]),
    dict(id='long', reference='Quiero preparar una lista de tareas para mañana y revisar primero los asuntos pendientes de esta semana, sin cambiar mis archivos.',
         checks=[['sin cambiar mis archivos']]),
    dict(id='digits', reference='El código de prueba es cuatro, siete, cero, dos.',
         checks=[['cuatro siete cero dos', '4 7 0 2', '4702']]),
]


def words(text):
    if not isinstance(text, str) or len(text) > 65536:
        raise ValueError('Text must be a bounded string')
    return re.findall(r'[^\W_]+', unicodedata.normalize('NFC', text).casefold(), re.UNICODE)


def alignment(reference, hypothesis):
    """Minimum word edits; ties prefer substitution, then deletion, then insertion.

    Case/punctuation ignored, accents retained. Numbers are NOT silently rewritten.
    The tie convention makes S/D/I reproducible; ambiguous alignments are possible.
    """
    ref, hyp = words(reference), words(hypothesis)
    if len(ref) > 512 or len(hyp) > 512:
        raise ValueError('Alignment limit: 512 words per side')
    costs = [list(range(len(hyp) + 1))]
    for i, a in enumerate(ref, 1):
        row = [i]
        for j, b in enumerate(hyp, 1):
            row.append(min(costs[-1][j-1] + (a != b), costs[-1][j] + 1, row[-1] + 1))
        costs.append(row)
    i, j = len(ref), len(hyp)
    edits = []
    while i or j:
        if i and j and costs[i][j] == costs[i-1][j-1] + (ref[i-1] != hyp[j-1]):
            edits.append(dict(op='equal' if ref[i-1] == hyp[j-1] else 'substitution',
                              reference=ref[i-1], hypothesis=hyp[j-1]))
            i -= 1
            j -= 1
        elif i and costs[i][j] == costs[i-1][j] + 1:
            edits.append(dict(op='deletion', reference=ref[i-1], hypothesis=None))
            i -= 1
        else:
            edits.append(dict(op='insertion', reference=None, hypothesis=hyp[j-1]))
            j -= 1
    counts = {name: sum(e['op'] == name for e in edits)
              for name in ('substitution', 'deletion', 'insertion')}
    errors = sum(counts.values())
    return dict(reference_words=len(ref), hypothesis_words=len(hyp), errors=errors,
                wer=errors / len(ref) if ref else (0.0 if not hyp else None),
                empty_unjustified=bool(ref) and not hyp, **counts, edits=list(reversed(edits)))


def check_phrases(hypothesis, checks):
    """Separate declared format alternatives; these NEVER modify strict WER."""
    hyp = words(hypothesis)
    return [dict(alternatives=alternatives, present=any(
        any(hyp[i:i+len(w)] == w for i in range(len(hyp)-len(w)+1))
        for w in (words(a) for a in alternatives) if w)) for alternatives in checks]


def reduced_context(frames, rate=16000):
    """Experimental duration-aware window: >= audio + 1 s, multiples of 256.

    A fixed 256 covers only 5.12 s at 50 encoder frames/s. Never apply it
    indiscriminately to long speech. This is a benchmark policy, not production.
    Full context (0) is used when a reduced padded window cannot cover the clip.
    """
    if rate != 16000 or not 0 < frames <= rate * 30:
        raise ValueError('Canonical PCM16 mono 16 kHz, duration (0,30] s required')
    needed = math.ceil((frames / rate + 1) * 50)
    context = max(256, math.ceil(needed / 256) * 256)
    return context if context < 1500 else 0
