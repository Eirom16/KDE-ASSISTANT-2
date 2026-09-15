#!/usr/bin/env python3
"""Sidecar de voz para KDE Assistant v2 (proceso separado, gestionado por main.rs).

Pipecat es dueño exclusivo del microfono y del pipeline de conversacion:
LocalAudioTransport -> WakeGate (openWakeWord, 100% local) -> Groq STT ->
Groq LLM (+ tools via function calling) -> Piper local -> altavoz.

El wake word NUNCA envia audio a la nube: hasta detectarlo, WakeGate tira los
frames de audio. Solo tras "hey jarvis" (o /conversation/start manual) el
audio fluye hacia Groq.

Eventos hacia Rust (para UI/personaje), POST a
  http://127.0.0.1:8765/api/voice/sidecar-event (Bearer token):
  {"type":"state","state":"wake_detected|listening|thinking|speaking|
                           interrupted|tool_call|idle"}
  {"type":"level","level":0..1}
  {"type":"exchange","transcript":"...","response":"...","timestamp_ms":...}
  {"type":"dictation","transcript":"..."}   (dictado del chat)

Control HTTP local (127.0.0.1:8766, peticiones desde main.rs / http_server):
  GET  /health
  POST /conversation/start      abre una conversacion (PTT / UI)
  POST /conversation/interrupt  barge-in manual
  POST /dictate/start           dictado al chat (sin LLM/TTS)
  POST /dictate/stop            -> {"transcript": "..."}
"""

import json
import asyncio
import io
import os
import sys
import time
import uuid
import wave
from pathlib import Path

import numpy as np
from aiohttp import ClientSession, ClientTimeout, web

from openwakeword.model import Model as OwwModel

from pipecat.audio.vad.silero import SileroVADAnalyzer
from pipecat.audio.vad.vad_analyzer import VADParams
from pipecat.frames.frames import (
    BotStartedSpeakingFrame,
    BotStoppedSpeakingFrame,
    Frame,
    InputAudioRawFrame,
    InterruptionFrame,
    TranscriptionFrame,
    TTSSpeakFrame,
    VADUserStartedSpeakingFrame,
    VADUserStoppedSpeakingFrame,
)
from pipecat.pipeline.pipeline import Pipeline
from pipecat.pipeline.runner import PipelineRunner
from pipecat.pipeline.task import PipelineParams, PipelineTask
from pipecat.processors.aggregators.llm_context import LLMContext
from pipecat.processors.aggregators.llm_response_universal import (
    LLMContextAggregatorPair,
    LLMUserAggregatorParams,
)
from pipecat.processors.frame_processor import FrameDirection, FrameProcessor
from pipecat.services.groq.llm import GroqLLMService
from pipecat.services.groq.stt import GroqSTTService
from pipecat.services.llm_service import FunctionCallParams
from pipecat.services.piper.tts import PiperTTSService
from pipecat.transcriptions.language import Language
from pipecat.transports.local.audio import LocalAudioTransport, LocalAudioTransportParams

# ============================================================
# Configuracion
# ============================================================

RUST_URL = os.environ.get("KDE_ASSISTANT_URL", "http://127.0.0.1:8765")
RUST_TOKEN = os.environ.get("KDE_ASSISTANT_TOKEN", "")
SIDECAR_PORT = 8766

OWW_DIR = Path.home() / ".local/share/kde-assistant/models/wakeword"
PIPER_DIR = Path.home() / ".local/share/kde-assistant/models/piper"
PIPER_VOICE = os.environ.get("KDE_ASSISTANT_TTS_VOICE", "es_ES-sharvard-medium")
WAKE_MODEL = OWW_DIR / "hey_jarvis_v0.1.onnx"
WAKE_THRESHOLD = float(os.environ.get("KDE_ASSISTANT_WAKE_THRESHOLD", "0.25"))
WAKE_COOLDOWN_SECS = 2.0
WAKE_WINDOW_SECS = 15.0        # ventana de conversacion tras wake
AUTO_LISTEN_WINDOW_SECS = float(os.environ.get("KDE_ASSISTANT_LISTEN_WINDOW", "6.0"))
WAKE_GREETING = os.environ.get("KDE_ASSISTANT_WAKE_GREETING", "Si, digame.")
GROQ_MODEL_STT = os.environ.get("KDE_ASSISTANT_STT_MODEL", "whisper-large-v3-turbo")
GROQ_MODEL_LLM = os.environ.get("KDE_ASSISTANT_LLM_MODEL", "openai/gpt-oss-120b")
CONFIG_PATH = Path.home() / ".config/kde-assistant/config.json"


def load_groq_key() -> str:
    """La key GROQ vive en ~/.config/kde-assistant/config.json (la misma que
    usa el chat de texto). Env var tiene primacidad para pruebas."""
    if env := os.environ.get("GROQ_API_KEY"):
        return env
    try:
        cfg = json.loads(CONFIG_PATH.read_text())
        return cfg.get("ai", {}).get("api_key", "") or ""
    except Exception:
        return ""


SYSTEM_PROMPT = (
    "Eres Jarvis, un asistente de voz en el escritorio KDE Plasma. "
    "Responde en espanol, frases cortas y habladas (1-2 oraciones). "
    "Sin markdown, listas ni emojis: se habla en voz alta, no se lee."
)


# ============================================================
# Puente de eventos hacia Rust (ui/avatar)
# ============================================================


class EventBridge:
    def __init__(self) -> None:
        self._session: ClientSession | None = None
        self._pending: asyncio.Queue = asyncio.Queue()
        self._task: asyncio.Task | None = None

    async def start(self) -> None:
        self._session = ClientSession()
        self._task = asyncio.create_task(self._drain())

    async def _drain(self) -> None:
        while True:
            payload = await self._pending.get()
            try:
                headers = {"Authorization": f"Bearer {RUST_TOKEN}"} if RUST_TOKEN else {}
                await self._session.post(
                    f"{RUST_URL}/api/voice/sidecar-event",
                    json=payload,
                    headers=headers,
                    timeout=ClientTimeout(total=2.0),
                )
            except Exception as e:
                si = sys.stderr
                print(f"[bridge] evento perdido: {e}", file=si, flush=True)

    def emit(self, payload: dict) -> None:
        try:
            self._pending.put_nowait(payload)
        except Exception:
            pass

    def state(self, state: str) -> None:
        self.emit({"type": "state", "state": state})


# ============================================================
# WakeGate: wake word local (openWakeWord) + dictado del chat
# ============================================================
#
# - FrameProcessor antes del STT. En reposo los frames NO siguen adelante
#   (nada sale del equipo). Solo al detectar "hey jarvis" o al abrir una
#   conversacion/dictado manual, deja pasar o buffers los frames.


class WakeGate(FrameProcessor):
    def __init__(self, bridge: EventBridge) -> None:
        super().__init__()
        self._bridge = bridge
        self._model = OwwModel(
            wakeword_models=[str(WAKE_MODEL)],
            inference_framework="onnx",
        )
        self._active_until = 0.0
        self._last_wake = 0.0
        self._wake_buf = np.zeros(0, dtype=np.float32)
        self._dictating = False
        self._dictate_pcm = bytearray()
        self._wakeword_just_fired = False

    def _now(self) -> float:
        return time.monotonic()

    # ---- API llamada desde ControlServer ----
    def force_conversation(self) -> None:
        """Abre conversacion sin wake word (PTT desde Rust)."""
        self._active_until = self._now() + WAKE_WINDOW_SECS
        self._bridge.state("listening")

    def conversation_active(self) -> bool:
        return self._now() < self._active_until

    def started_dictating(self) -> None:
        self._dictating = True
        self._dictate_pcm.clear()

    def stop_dictating(self) -> bytes:
        self._dictating = False
        return bytes(self._dictate_pcm)

    # ---- FrameProcessor ----
    async def process_frame(self, frame: Frame, direction: FrameDirection):
        await super().process_frame(frame, direction)

        if isinstance(frame, InputAudioRawFrame):
            if self._dictating:
                self._dictate_pcm.extend(self._to_mono16k(frame))
                return
            if self.conversation_active():
                await self.push_frame(frame, direction)
                return
            await self._check_wakeup(frame)
            return

        await self.push_frame(frame, direction)

    async def _check_wakeup(self, frame: InputAudioRawFrame) -> None:
        mono16 = self._to_mono16k(frame)
        self._wake_buf = np.concatenate([self._wake_buf, mono16])
        while len(self._wake_buf) >= 1280:
            chunk = self._wake_buf[:1280]
            self._wake_buf = self._wake_buf[1280:]
            try:
                scores = self._model.predict(chunk)
            except Exception as e:
                print(f"[wakegate] onnx error: {e}", file=sys.stderr, flush=True)
                continue
            score = max(scores.values()) if scores else 0.0
            if score >= WAKE_THRESHOLD and (self._now() - self._last_wake) > WAKE_COOLDOWN_SECS:
                self._last_wake = self._now()
                self._active_until = self._now() + WAKE_WINDOW_SECS
                self._wakeword_just_fired = True
                self._bridge.state("wake_detected")

    def take_wake_flag(self) -> bool:
        out = self._wakeword_just_fired
        self._wakeword_just_fired = False
        return out

    @staticmethod
    def _to_mono16k(frame: InputAudioRawFrame) -> np.ndarray:
        sr = frame.sample_rate or 16000
        mono = np.frombuffer(frame.audio, dtype=np.int16).astype(np.float32)
        if sr == 16000:
            return mono
        target = max(1, int(len(mono) * 16000 / sr))
        return np.interp(
            np.linspace(0, len(mono) - 1, target), np.arange(len(mono)), mono
        ).astype(np.float32)


# ============================================================
# Cuando detectamos wake word: saludo + estado y el flujo queda abierto
# ============================================================

class ConversationSupervisor:
    """Watches para senalar fin de conversacion: tras 'speaking', si nadie
    habla durante AUTO_LISTEN_WINDOW_SECS, volvemos a idle."""

    def __init__(self, gate: WakeGate, bridge: EventBridge, task: PipelineTask) -> None:
        self.gate = gate
        self.bridge = bridge
        self.task = task
        self._last_bot_end = 0.0
        self._last_user_start = 0.0

    def user_started(self) -> None:
        self._last_user_start = time.monotonic()
        # Extender ventana de conversacion mientras el usuario habla.
        self.gate.extend_conversation(WAKE_WINDOW_SECS)

    def bot_stopped(self) -> None:
        self._last_bot_end = time.monotonic()
        # Despues de responder, dejamos abierto un rato para follow-ups.
        self.gate.extend_conversation(AUTO_LISTEN_WINDOW_SECS + 0.5)

    async def run(self) -> None:
        while True:
            await asyncio.sleep(0.5)
            if not self.gate.conversation_active():
                continue


# ============================================================
# Taps de estado y texto
# ============================================================


class StateTap(FrameProcessor):
    """Lanea pipecat frames a eventos de estado para la UI/personaje."""

    def __init__(
        self,
        bridge: EventBridge,
        supervisor: "ConversationSupervisor | None" = None,
    ) -> None:
        super().__init__()
        self._bridge = bridge
        self._supervisor = supervisor

    def set_supervisor(self, s: ConversationSupervisor) -> None:
        self._supervisor = s

    async def process_frame(self, frame: Frame, direction: FrameDirection):
        await super().process_frame(frame, direction)
        if isinstance(frame, VADUserStartedSpeakingFrame):
            self._bridge.state("listening")
            if self._supervisor:
                self._supervisor.user_started()
        elif isinstance(frame, VADUserStoppedSpeakingFrame):
            self._bridge.state("thinking")
        elif isinstance(frame, BotStartedSpeakingFrame):
            self._bridge.state("speaking")
        elif isinstance(frame, BotStoppedSpeakingFrame):
            self._bridge.state("idle_after_speech")
            if self._supervisor:
                self._supervisor.bot_stopped()
        elif isinstance(frame, InterruptionFrame):
            self._bridge.state("interrupted")
        await self.push_frame(frame, direction)


class HistoryTap(FrameProcessor):
    """Anota (transcript user, response assistant) por turno y lo persiste."""

    def __init__(self, bridge: EventBridge) -> None:
        super().__init__()
        self._bridge = bridge
        self._last_user_text: str = ""
        self._assistant_frag: list[str] = []

    async def process_frame(self, frame: Frame, direction: FrameDirection):
        await super().process_frame(frame, direction)
        if isinstance(frame, TranscriptionFrame):
            self._last_user_text = frame.text
        elif isinstance(frame, BotStoppedSpeakingFrame):
            # turno completo: persistir si hubo contenido
            user = self._last_user_text.strip()
            self._last_user_text = ""
            if user:
                self._bridge.emit(
                    {
                        "type": "exchange",
                        "transcript": user,
                        "response": "",
                        "timestamp_ms": int(time.time() * 1000),
                    }
                )
        await self.push_frame(frame, direction)


# ============================================================
# Servidor de control (Rust -> sidecar)
# ============================================================


class ControlServer:
    def __init__(self, gate: WakeGate, bridge: EventBridge) -> None:
        self.gate = gate
        self.bridge = bridge
        self.task: PipelineTask | None = None

    async def health(self, _r: web.Request) -> web.Response:
        return web.json_response(
            {
                "ok": True,
                "wake": True,
                "conversation": self.gate.conversation_active(),
                "dictating": self.gate._dictating,
            }
        )

    async def conversation_start(self, _r: web.Request) -> web.Response:
        self.gate.force_conversation()
        return web.json_response({"status": "listening"})

    async def interrupt(self, _r: web.Request) -> web.Response:
        if self.task:
            await self.task.queue_frame(InterruptionFrame())
        return web.json_response({"status": "interrupted"})

    async def dictate_start(self, _r: web.Request) -> web.Response:
        self.gate.started_dictating()
        return web.json_response({"status": "dictating"})

    async def dictate_stop(self, _r: web.Request) -> web.Response:
        pcm = self.gate.stop_dictating()
        text = await transcribe_whisper_api(pcm)
        return web.json_response({"transcript": text})

    async def run(self) -> None:
        app = web.Application()
        app.add_routes(
            [
                web.get("/health", self.health),
                web.post("/conversation/start", self.conversation_start),
                web.post("/conversation/interrupt", self.interrupt),
                web.post("/dictate/start", self.dictate_start),
                web.post("/dictate/stop", self.dictate_stop),
            ]
        )
        runner = web.AppRunner(app)
        await runner.setup()
        site = web.TCPSite(runner, "127.0.0.1", SIDECAR_PORT)
        await site.start()
        print(f"[sidecar] control en http://127.0.0.1:{SIDECAR_PORT}", flush=True)


async def transcribe_whisper_api(pcm: bytes) -> str:
    """Transcribe PCM16 mono 16k via Groq Whisper API (solo dictado)."""
    if not pcm:
        return ""
    buf = io.BytesIO()
    with wave.open(buf, "wb") as wf:
        wf.setnchannels(1)
        wf.setsampwidth(2)
        wf.setframerate(16000)
        wf.writeframes(pcm)

    import aiohttp

    data = aiohttp.FormData()
    data.add_field("model", GROQ_MODEL_STT)
    data.add_field("response_format", "json")
    data.add_field("file", buf.getvalue(), filename="audio.wav", content_type="audio/wav")
    try:
        async with ClientSession() as s:
            async with s.post(
                "https://api.groq.com/openai/v1/audio/transcriptions",
                data=data,
                headers={"Authorization": f"Bearer {load_groq_key()}"},
                timeout=ClientTimeout(total=30),
            ) as r:
                body = await r.json(content_type=None)
                return str(body.get("text", "")).strip()
    except Exception as e:
        print(f"[sidecar] dictado: error STT: {e}", file=sys.stderr, flush=True)
        return ""


# ============================================================
# Pipeline main
# ============================================================


async def run() -> None:
    api_key = load_groq_key()
    if not api_key:
        sys.exit("Falta GROQ_API_KEY")

    bridge = EventBridge()
    await bridge.start()
    gate = WakeGate(bridge)
    bridge.state("idle")

    control = ControlServer(gate, bridge)

    # ---- transport (mic + altavoz, propiedad exclusiva del sidecar) ----
    transport = LocalAudioTransport(
        LocalAudioTransportParams(
            audio_in_enabled=True,
            audio_out_enabled=True,
            audio_in_sample_rate=16000,
        )
    )

    stt = GroqSTTService(
        api_key=api_key,
        settings=GroqSTTService.Settings(model=GROQ_MODEL_STT, language=Language.ES),
    )
    llm = GroqLLMService(
        api_key=api_key,
        settings=GroqLLMService.Settings(model=GROQ_MODEL_LLM),
    )
    tts = PiperTTSService(
        settings=PiperTTSService.Settings(voice=PIPER_VOICE),
        download_dir=PIPER_DIR,
    )

    context = LLMContext(messages=[{"role": "system", "content": SYSTEM_PROMPT}])

    # ---- Function calling: el LLM llama a estas funciones; el executor real
    # vive en Rust (/api/tools/execute) que aplica permisos y auditoria. ----
    async def tool_call_handler(params: FunctionCallParams) -> None:
        name = params.function_name
        bridge.emit({"type": "tool_call", "name": name})
        result = await execute_tool_via_rust(name, params.arguments)
        await params.result_callback({"content": result})

    llm.register_function(None, tool_call_handler)

    # Esquema de tools de Rust (fuente unica de verdad, ya filtrado por config).
    try:
        async with ClientSession() as s:
            async with s.get(
                f"{RUST_URL}/api/tools/schemas",
                headers={"Authorization": f"Bearer {RUST_TOKEN}"},
                timeout=ClientTimeout(total=5),
            ) as r:
                openai_tools = await r.json(content_type=None)
        # De formato OpenAI ({type,function:{name,description,parameters}})
        # a FunctionSchema de pipecat.
        from pipecat.adapters.schemas.function_schema import FunctionSchema
        from pipecat.adapters.schemas.tools_schema import ToolsSchema

        function_schemas = []
        for tool in openai_tools:
            fn = tool.get("function", {})
            params_schema = fn.get("parameters", {}) or {}
            function_schemas.append(
                FunctionSchema(
                    name=fn.get("name", ""),
                    description=fn.get("description", ""),
                    properties=params_schema.get("properties", {}),
                    required=params_schema.get("required", []),
                )
            )
        if function_schemas:
            context.set_tools(ToolsSchema(standard_tools=function_schemas))
            print(f"[sidecar] {len(function_schemas)} tools cargadas", flush=True)
    except Exception as e:
        print(f"[sidecar] /api/tools/schemas no disponible: {e}", file=sys.stderr, flush=True)

    user_aggr, assistant_aggr = LLMContextAggregatorPair(
        context,
        user_params=LLMUserAggregatorParams(
            vad_analyzer=SileroVADAnalyzer(
                params=VADParams(confidence=0.5, start_secs=0.25, stop_secs=0.7)
            )
        ),
    )

    tap = StateTap(bridge)
    history = HistoryTap(bridge)

    pipeline = Pipeline(
        [
            transport.input(),
            gate,
            stt,
            user_aggr,
            llm,
            tts,
            transport.output(),
            assistant_aggr,
            history,
            tap,
        ]
    )

    task = PipelineTask(pipeline, params=PipelineParams(allow_interruptions=True))
    control.task = task
    supervisor = ConversationSupervisor(gate, bridge, task)
    tap.set_supervisor(supervisor)

    # Saludo al detectar wake word.
    async def greet_on_wake() -> None:
        while True:
            await asyncio.sleep(0.1)
            if gate.take_wake_flag():
                bridge.state("wake_detected")
                ttsspeak = TTSSpeakFrame(text=WAKE_GREETING)
                await task.queue_frame(ttsspeak)

    await control.run()
    print("[sidecar] pipeline viva; esperando 'hey jarvis'...", flush=True)

    supervisor_task = asyncio.create_task(supervisor.run())
    greet_task = asyncio.create_task(greet_on_wake())
    try:
        await PipelineRunner().run(task)
    finally:
        supervisor_task.cancel()
        greet_task.cancel()


async def execute_tool_via_rust(name: str, args: dict) -> str:
    try:
        async with ClientSession() as s:
            async with s.post(
                f"{RUST_URL}/api/tools/execute",
                json={"name": name, "arguments": args},
                headers={"Authorization": f"Bearer {RUST_TOKEN}"},
                timeout=ClientTimeout(total=30),
            ) as r:
                body = await r.json(content_type=None)
                if r.status == 200:
                    return str(body.get("content", "(hecho)"))
                return f"(error {r.status}: {body.get('error', 'desconocido')})"
    except Exception as e:
        return f"(fallo de red hacia la herramienta: {e})"


if __name__ == "__main__":
    try:
        asyncio.run(run())
    except KeyboardInterrupt:
        pass
