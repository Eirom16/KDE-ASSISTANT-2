// CharacterData.js — GENERADO por tools/blobatar-export (no editar a mano)
// Identidad del Assistant Character. Fuente: blobatar 2.7.0 (MIT), seed
// "kde-assistant", hue 225, traits shape 0.65 (capsule). Ver
// assets/character/layout.json para el dump completo y ATTRIBUTION.md.
.pragma library

var VIEW_BOX = 100.0

var body = {"fill":"#0094bb","eyeFill":"#061116","capY":50.7,"r":19.65,"left":38.17,"right":62.87,"top":31.05,"bottom":70.36}

// Ojos base: path SVG absoluto (viewBox 100) + frame (cx, cy, rx, ry, rot).
var eyes = [{"d":"M44.66 48.15C44.48 52.82 44.2 53.38 42.05 53.3C39.9 53.22 39.65 52.64 39.83 47.97C40.01 43.31 40.29 42.75 42.44 42.83C44.59 42.91 44.83 43.49 44.66 48.15Z","cx":42.243,"cy":48.064,"rx":2.415,"ry":5.238,"rot":2.16,"fill":"#061116"},{"d":"M61.69 47.43C61.8 53.47 61.5 54.21 58.97 54.26C56.44 54.3 56.12 53.57 56.01 47.53C55.9 41.49 56.2 40.74 58.73 40.7C61.26 40.65 61.58 41.39 61.69 47.43Z","cx":58.852,"cy":47.478,"rx":2.84,"ry":6.781,"rot":-1.02,"fill":"#061116"}]

var EXPRESSION_NAMES = ["idle","happy","sad","mad","surprised","wink","sleepy","smug","unsure","scared","love","shy","sick","thinking"]

// Pose = 13 canales + hot (color de tinte o null).
// idle equivale a: esx=1, esy=1, resto 0.
var poses = {
    "idle": {
        "esx": 1,
        "esy": 1,
        "tilt": 0,
        "edy": 0,
        "edx": 0,
        "esx2": 0,
        "esy2": 0,
        "tilt2": 0,
        "edy2": 0,
        "lock": 0,
        "heat": 0,
        "shake": 0,
        "rock": 0,
        "bdy": 0,
        "hot": null
    },
    "happy": {
        "esx": 1.72,
        "esy": 0.3,
        "tilt": 8,
        "edy": -1.5,
        "edx": 1.5,
        "esx2": 0.08,
        "esy2": 0.05,
        "tilt2": -16,
        "edy2": 0,
        "lock": 1,
        "heat": 0,
        "shake": 0,
        "rock": 0,
        "bdy": -2.2,
        "hot": null
    },
    "sad": {
        "esx": 0.6,
        "esy": 0.56,
        "tilt": 26,
        "edy": 3.6,
        "edx": 1.9,
        "esx2": -0.05,
        "esy2": -0.07,
        "tilt2": -7,
        "edy2": 0,
        "lock": 1,
        "heat": 0,
        "shake": 0,
        "rock": 0,
        "bdy": 2.6,
        "hot": null
    },
    "mad": {
        "esx": 1.85,
        "esy": 0.26,
        "tilt": -33,
        "edy": 0.4,
        "edx": 0.6,
        "esx2": 0,
        "esy2": -0.03,
        "tilt2": 5,
        "edy2": 0,
        "lock": 1,
        "heat": 0.62,
        "shake": 0.55,
        "rock": 0,
        "bdy": 0.8,
        "hot": "#a87173"
    },
    "surprised": {
        "esx": 1.34,
        "esy": 1.2,
        "tilt": -6,
        "edy": -1.05,
        "edx": 0.5,
        "esx2": 0.05,
        "esy2": 0.07,
        "tilt2": 3,
        "edy2": 0,
        "lock": 1,
        "heat": 0,
        "shake": 0,
        "rock": 0,
        "bdy": -1.4,
        "hot": null
    },
    "wink": {
        "esx": 1.32,
        "esy": 0.76,
        "tilt": 5,
        "edy": -0.6,
        "edx": 0.8,
        "esx2": 0.26,
        "esy2": -0.56,
        "tilt2": -11,
        "edy2": 0,
        "lock": 1,
        "heat": 0,
        "shake": 0,
        "rock": 0,
        "bdy": -1.1,
        "hot": null
    },
    "sleepy": {
        "esx": 1.14,
        "esy": 0.22,
        "tilt": 0,
        "edy": 2.4,
        "edx": 0.3,
        "esx2": -0.04,
        "esy2": 0.03,
        "tilt2": 4,
        "edy2": 0,
        "lock": 1,
        "heat": 0,
        "shake": 0,
        "rock": 0,
        "bdy": 1.2,
        "hot": null
    },
    "smug": {
        "esx": 1.3,
        "esy": 0.42,
        "tilt": 18,
        "edy": -0.5,
        "edx": 0.5,
        "esx2": 0.06,
        "esy2": -0.06,
        "tilt2": -36,
        "edy2": 0,
        "lock": 1,
        "heat": 0,
        "shake": 0,
        "rock": 0,
        "bdy": -1,
        "hot": null
    },
    "unsure": {
        "esx": 0.95,
        "esy": 1.02,
        "tilt": 4,
        "edy": -0.2,
        "edx": 0.3,
        "esx2": 0.24,
        "esy2": -0.44,
        "tilt2": -18,
        "edy2": 0,
        "lock": 1,
        "heat": 0,
        "shake": 0,
        "rock": 0,
        "bdy": 0,
        "hot": null
    },
    "scared": {
        "esx": 0.78,
        "esy": 0.96,
        "tilt": -12,
        "edy": -1.5,
        "edx": -0.8,
        "esx2": -0.04,
        "esy2": 0.05,
        "tilt2": 4,
        "edy2": 0,
        "lock": 1,
        "heat": 0,
        "shake": 0.35,
        "rock": 0,
        "bdy": -0.6,
        "hot": null
    },
    "love": {
        "esx": 0.86,
        "esy": 1.28,
        "tilt": -14,
        "edy": -0.5,
        "edx": -0.35,
        "esx2": 0.05,
        "esy2": 0.06,
        "tilt2": 6,
        "edy2": 0,
        "lock": 1,
        "heat": 0.6,
        "shake": 0,
        "rock": 0,
        "bdy": -1.6,
        "hot": "#aa80a5"
    },
    "shy": {
        "esx": 0.62,
        "esy": 0.5,
        "tilt": 10,
        "edy": 1.4,
        "edx": -0.2,
        "esx2": -0.05,
        "esy2": -0.04,
        "tilt2": -8,
        "edy2": 0,
        "lock": 1,
        "heat": 0.55,
        "shake": 0,
        "rock": 0,
        "bdy": 0.9,
        "hot": "#9e8fa3"
    },
    "sick": {
        "esx": 1.25,
        "esy": 0.34,
        "tilt": 20,
        "edy": 1.8,
        "edx": 0.8,
        "esx2": 0.05,
        "esy2": -0.05,
        "tilt2": -6,
        "edy2": 0,
        "lock": 1,
        "heat": 0.6,
        "shake": 0.18,
        "rock": 0,
        "bdy": 1.4,
        "hot": "#439d83"
    },
    "thinking": {
        "esx": 1.15,
        "esy": 0.62,
        "tilt": 0,
        "edy": 4.2,
        "edx": 0.4,
        "esx2": 0.02,
        "esy2": 0.06,
        "tilt2": 0,
        "edy2": -8.4,
        "lock": 1,
        "heat": 0,
        "shake": 0,
        "rock": 0.8,
        "bdy": -0.4,
        "hot": null
    }
}

function isValidExpression(name) {
    return poses[name] !== undefined
}

function poseOf(name) {
    return poses[name] || poses.idle
}

// Mezcla lineal de canales entre dos poses (para transicion suave).
// t en [0,1]: 0 = a, 1 = b.
function lerpPose(a, b, t) {
    var out = {}
    var names = ["esx","esy","tilt","edy","edx","esx2","esy2","tilt2","edy2","lock","heat","shake","rock","bdy"]
    for (var i = 0; i < names.length; i++) {
        var k = names[i]
        out[k] = a[k] + (b[k] - a[k]) * t
    }
    // El tinte cruza por el de destino desde el inicio (crossfade de color).
    out.hot = t >= 0.5 ? b.hot : (t > 0 ? a.hot : a.hot)
    if (b.hot === null && t < 1) out.hot = a.hot
    if (a.hot === null && b.hot !== null && t >= 1) out.hot = b.hot
    return out
}

// Mezcla dos colores #rrggbb por t en [0,1].
function mixColor(hexA, hexB, t) {
    var pa = parseInt(hexA.slice(1), 16)
    var pb = parseInt(hexB.slice(1), 16)
    var ar = (pa >> 16) & 255, ag = (pa >> 8) & 255, ab = pa & 255
    var br = (pb >> 16) & 255, bg = (pb >> 8) & 255, bb = pb & 255
    var r = Math.round(ar + (br - ar) * t)
    var g = Math.round(ag + (bg - ag) * t)
    var b = Math.round(ab + (bb - ab) * t)
    return "#" + ((1 << 24) + (r << 16) + (g << 8) + b).toString(16).slice(1)
}

// Color del cuerpo para una pose. OJO: `hot` ya es el color FINAL mezclado
// por la propia libreria ("a finished colour"); no hay que reaplicar heat.
function bodyColor(pose) {
    return pose.hot ? pose.hot : body.fill
}
