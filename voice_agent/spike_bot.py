#!/usr/bin/env python3
"""Spike P0: bot de voz bidireccional con Pipecat 1.10.

mic (LocalAudioTransport) -> Silero VAD -> Groq Whisper STT -> Groq LLM -> Piper TTS
con interrupciones reales (barge-in) gestionadas por el framework.

    cd voice_agent && source .venv/bin/activate
    GROQ_API_KEY=... python spike_bot.py
"""

import asyncio
import os
import sys
from pathlib import Path

from pipecat.audio.vad.silero import SileroVADAnalyzer
from pipecat.audio.vad.vad_analyzer import VADParams
from pipecat.pipeline.pipeline import Pipeline
from pipecat.pipeline.runner import PipelineRunner
from pipecat.pipeline.task import PipelineParams, PipelineTask
from pipecat.processors.aggregators.llm_context import LLMContext
from pipecat.frames.frames import LLMRunFrame
from pipecat.processors.aggregators.llm_response_universal import (
    LLMContextAggregatorPair,
    LLMUserAggregatorParams,
)
from pipecat.services.groq.llm import GroqLLMService
from pipecat.services.groq.stt import GroqSTTService
from pipecat.services.piper.tts import PiperTTSService
from pipecat.transcriptions.language import Language
from pipecat.transports.local.audio import LocalAudioTransport, LocalAudioTransportParams

PIPER_DIR = Path(os.path.expanduser("~/.local/share/kde-assistant/models/piper"))
PIPER_VOICE = "es_ES-sharvard-medium"


async def main() -> None:
    api_key = os.environ.get("GROQ_API_KEY")
    if not api_key:
        print("Falta GROQ_API_KEY en el entorno", file=sys.stderr)
        sys.exit(1)

    # Silero VAD: fin de turno tras 700ms de silencio real (no energia cruda),
    # min 250ms de voz para arrancar (evita golpes/teclado).
    vad = SileroVADAnalyzer(
        params=VADParams(confidence=0.5, start_secs=0.25, stop_secs=0.7)
    )

    transport = LocalAudioTransport(
        LocalAudioTransportParams(
            audio_in_enabled=True,
            audio_out_enabled=True,
        )
    )

    stt = GroqSTTService(
        api_key=api_key,
        settings=GroqSTTService.Settings(
            model="whisper-large-v3-turbo",
            language=Language.ES,
        ),
    )
    llm = GroqLLMService(
        api_key=api_key,
        settings=GroqLLMService.Settings(model="openai/gpt-oss-120b"),
    )
    tts = PiperTTSService(
        settings=PiperTTSService.Settings(voice=PIPER_VOICE),
        download_dir=PIPER_DIR,
    )

    context = LLMContext(
        messages=[
            {
                "role": "system",
                "content": (
                    "Eres Jarvis, un asistente de voz en un escritorio KDE Plasma. "
                    "Responde en espanol con frases cortas habladas, una o dos "
                    "oraciones maximo. Sin markdown, sin listas: solo voz hablada."
                ),
            }
        ]
    )
    user_aggr, assistant_aggr = LLMContextAggregatorPair(
        context,
        user_params=LLMUserAggregatorParams(vad_analyzer=vad),
    )

    pipeline = Pipeline(
        [
            transport.input(),
            stt,
            user_aggr,
            llm,
            tts,
            transport.output(),
            assistant_aggr,
        ]
    )

    task = PipelineTask(
        pipeline,
        params=PipelineParams(
            allow_interruptions=True,
        ),
    )

    @transport.event_handler("on_first_participant_joined")
    async def on_joined(transport, participant):
        print(">>> Listo. Habla cuando quieras (Ctrl+C para salir).", flush=True)
        await task.queue_frames([LLMRunFrame()])

    runner = PipelineRunner(handle_sigint=True)
    await runner.run(task)


if __name__ == "__main__":
    asyncio.run(main())
