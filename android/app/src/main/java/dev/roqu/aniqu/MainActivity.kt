// AniQu für Android: Oberfläche in Compose, alles andere (hianime, Fortschritt, Listen, AniSkip) kommt aus
// Rust (aniqu-core über aniqu-ffi, Kotlin-Anbindung in uniffi.aniqu_ffi, erzeugt von build-rust.sh).
// Texte in der Oberfläche: Englisch.
package dev.roqu.aniqu

import android.app.Activity
import android.content.pm.ActivityInfo
import android.net.Uri
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.BackHandler
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyGridScope
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.runtime.snapshots.SnapshotStateList
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import androidx.media3.common.C
import androidx.media3.common.ForwardingPlayer
import androidx.media3.common.MediaItem
import androidx.media3.common.MimeTypes
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.DefaultDataSource
import androidx.media3.datasource.DefaultHttpDataSource
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import androidx.media3.ui.DefaultTimeBar
import androidx.media3.ui.PlayerView
import coil3.compose.AsyncImage
import kotlinx.coroutines.async
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import uniffi.aniqu_ffi.*
import java.io.File

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        init(filesDir.path, cacheDir.path)
        enableEdgeToEdge()
        val link = intent?.dataString
        setContent {
            MaterialTheme(colorScheme = darkColorScheme()) {
                Surface(Modifier.fillMaxSize()) { App(link) }
            }
        }
    }
}

sealed interface Page {
    data object Home : Page
    data class Show(val anime: Anime) : Page
    /** `episodes` null = beim Öffnen laden (z. B. aus „Continue watching“). */
    data class Watch(val anime: Anime, val number: UInt, val dub: Boolean, val episodes: List<Episode>?) : Page
}

fun bare(id: String) = Anime(id = id, title = "", poster = null, sub = null, dub = null, kind = null, duration = null)

fun message(e: Exception) = (e as? AniException.Failed)?.msg ?: e.message ?: "Something went wrong"

/** Fokus sichtbar machen (Android TV, Fernbedienung): weißer Rahmen und leicht größer. Vor `clickable` setzen. */
@Composable
fun Modifier.tvFocus(shape: Shape = RoundedCornerShape(10.dp), grow: Float = 1.05f): Modifier {
    var focused by remember { mutableStateOf(false) }
    val scale by animateFloatAsState(if (focused) grow else 1f, label = "focus")
    return onFocusChanged { focused = it.isFocused }
        .graphicsLayer { scaleX = scale; scaleY = scale }
        .then(if (focused) Modifier.border(3.dp, Color.White, shape) else Modifier)
}

@Composable
fun App(link: String?) {
    val stack = remember { mutableStateListOf<Page>(Page.Home) }
    LaunchedEffect(link) {
        val ids = link?.let { parseLink(it) } ?: return@LaunchedEffect
        stack.add(Page.Show(bare(ids[0])))
    }
    BackHandler(stack.size > 1) { stack.removeAt(stack.lastIndex) }
    // key: jede Seite frisch (eigener Zustand, Player wird beim Verlassen freigegeben).
    when (val page = stack.last()) {
        Page.Home -> key(page) {
            HomePage(
                open = { stack.add(Page.Show(it)) },
                resume = { stack.add(Page.Watch(it.anime, it.episode, dub(it.anime.id), null)) },
            )
        }
        is Page.Show -> key(page) { ShowPage(page.anime) { stack.add(it) } }
        is Page.Watch -> key(page) { WatchPage(page, stack) }
    }
}

@Composable
fun PosterCard(a: Anime, label: String? = null, fraction: Double? = null, onClick: () -> Unit) {
    Column(Modifier.tvFocus().clickable(onClick = onClick).padding(4.dp)) {
        Box {
            AsyncImage(
                model = a.poster,
                contentDescription = a.title,
                contentScale = ContentScale.Crop,
                modifier = Modifier.fillMaxWidth().aspectRatio(2f / 3f).clip(RoundedCornerShape(8.dp))
                    .background(MaterialTheme.colorScheme.surfaceVariant),
            )
            if (fraction != null) {
                LinearProgressIndicator(
                    progress = { fraction.toFloat() },
                    modifier = Modifier.fillMaxWidth().align(Alignment.BottomCenter),
                )
            }
        }
        Text(a.title, maxLines = 2, overflow = TextOverflow.Ellipsis, style = MaterialTheme.typography.bodySmall,
            modifier = Modifier.padding(top = 4.dp))
        if (label != null) {
            Text(label, style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

fun LazyGridScope.header(text: String) {
    item(span = { GridItemSpan(maxLineSpan) }) {
        Text(text, style = MaterialTheme.typography.titleMedium, modifier = Modifier.padding(top = 16.dp))
    }
}

@Composable
fun HomePage(open: (Anime) -> Unit, resume: (Resume) -> Unit) {
    val scope = rememberCoroutineScope()
    var query by remember { mutableStateOf("") }
    // Suche erst als Knopf: ein fokussiertes Textfeld öffnet sofort die Tastatur (auf dem TV schon beim Durchnavigieren).
    var editing by remember { mutableStateOf(false) }
    var found by remember { mutableStateOf<List<Anime>?>(null) }
    var popular by remember { mutableStateOf(savedTrending()) }
    var error by remember { mutableStateOf<String?>(null) }
    val going = remember { continueWatching(12u) }
    val saved = remember { watchlist() }
    // Zurück: erst Tastatur/Suchergebnisse schließen, dann erst die App.
    BackHandler(editing || found != null) {
        if (editing) editing = false else { found = null; query = "" }
    }
    LaunchedEffect(Unit) {
        try { popular = trending() } catch (e: Exception) { error = message(e) }
    }
    LazyVerticalGrid(
        columns = GridCells.Adaptive(110.dp),
        contentPadding = PaddingValues(12.dp),
        horizontalArrangement = Arrangement.spacedBy(10.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
        modifier = Modifier.safeDrawingPadding(),
    ) {
        item(span = { GridItemSpan(maxLineSpan) }) {
            if (editing) {
                val field = remember { FocusRequester() }
                LaunchedEffect(Unit) { field.requestFocus() }
                OutlinedTextField(
                    value = query,
                    onValueChange = { query = it },
                    placeholder = { Text("Search") },
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                    keyboardActions = KeyboardActions(onSearch = {
                        editing = false
                        if (query.isBlank()) { found = null; return@KeyboardActions }
                        scope.launch {
                            error = null
                            try { found = search(query.trim()) } catch (e: Exception) { error = message(e) }
                        }
                    }),
                    modifier = Modifier.fillMaxWidth().focusRequester(field),
                )
            } else {
                OutlinedButton(onClick = { editing = true }, modifier = Modifier.fillMaxWidth().tvFocus(RoundedCornerShape(50))) {
                    Text(query.ifEmpty { "Search" }, modifier = Modifier.weight(1f))
                }
            }
        }
        error?.let { e -> item(span = { GridItemSpan(maxLineSpan) }) { Text(e, color = MaterialTheme.colorScheme.error) } }
        val results = found
        if (results != null) {
            header(if (results.isEmpty()) "Nothing found" else "Results")
            items(results, key = { "s" + it.id }) { PosterCard(it) { open(it) } }
            return@LazyVerticalGrid
        }
        if (going.isNotEmpty()) {
            header("Continue watching")
            items(going, key = { "c" + it.anime.id }) { r ->
                PosterCard(r.anime, "Episode ${r.episode}", r.fraction) { resume(r) }
            }
        }
        if (saved.isNotEmpty()) {
            header("Watchlist")
            items(saved, key = { "w" + it.id }) { PosterCard(it) { open(it) } }
        }
        val ids = saved.map { it.id }.toSet()
        header("Trending")
        items(popular.filter { it.id !in ids }, key = { "t" + it.id }) { PosterCard(it) { open(it) } }
    }
}

@Composable
fun ShowPage(start: Anime, play: (Page.Watch) -> Unit) {
    var anime by remember { mutableStateOf(start) }
    var info by remember { mutableStateOf<Details?>(null) }
    var list by remember { mutableStateOf<List<Episode>?>(null) }
    var error by remember { mutableStateOf<String?>(null) }
    var isDub by remember { mutableStateOf(dub(start.id)) }
    var listed by remember { mutableStateOf(onWatchlist(start.id)) }
    var more by remember { mutableStateOf(false) }
    LaunchedEffect(start.id) {
        try {
            coroutineScope {
                val d = async { details(start.id) }
                val e = async { episodes(start.id) }
                info = d.await().also { anime = it.anime }
                list = e.await()
            }
        } catch (e: Exception) { error = message(e) }
    }
    LazyColumn(Modifier.safeDrawingPadding(), contentPadding = PaddingValues(12.dp)) {
        item {
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                AsyncImage(
                    model = anime.poster,
                    contentDescription = null,
                    contentScale = ContentScale.Crop,
                    modifier = Modifier.width(120.dp).aspectRatio(2f / 3f).clip(RoundedCornerShape(8.dp))
                        .background(MaterialTheme.colorScheme.surfaceVariant),
                )
                Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    Text(anime.title, style = MaterialTheme.typography.titleLarge)
                    info?.let {
                        Text((it.info + it.genres).joinToString(" · "), style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                }
            }
        }
        item {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp),
                modifier = Modifier.padding(vertical = 8.dp)) {
                FilterChip(selected = !isDub, onClick = { isDub = false; setDub(start.id, false) }, label = { Text("Sub") },
                    modifier = Modifier.tvFocus())
                FilterChip(selected = isDub, onClick = { isDub = true; setDub(start.id, true) }, label = { Text("Dub") },
                    enabled = anime.dub != null, modifier = Modifier.tvFocus())
                Spacer(Modifier.weight(1f))
                TextButton(onClick = { listed = toggleWatchlist(anime) }, modifier = Modifier.tvFocus(RoundedCornerShape(50))) {
                    Text(if (listed) "On watchlist" else "Add to watchlist")
                }
            }
        }
        info?.description?.takeIf { it.isNotEmpty() }?.let { d ->
            item {
                Text(d, maxLines = if (more) Int.MAX_VALUE else 4, overflow = TextOverflow.Ellipsis,
                    style = MaterialTheme.typography.bodyMedium, modifier = Modifier.tvFocus(grow = 1f).clickable { more = !more }.padding(4.dp))
            }
        }
        error?.let { e -> item { Text(e, color = MaterialTheme.colorScheme.error) } }
        val eps = list
        if (eps == null && error == null) item { LinearProgressIndicator(Modifier.fillMaxWidth().padding(top = 16.dp)) }
        if (eps != null) {
            itemsIndexed(eps, key = { _, e -> e.id }) { _, ep ->
                val ok = available(anime, ep.number, isDub)
                ListItem(
                    headlineContent = { Text("${ep.number}. ${ep.title}", maxLines = 2, overflow = TextOverflow.Ellipsis) },
                    modifier = Modifier.alpha(if (ok) 1f else 0.4f).tvFocus(grow = 1f)
                        .clickable(enabled = ok) { play(Page.Watch(anime, ep.number, isDub, eps)) },
                )
            }
        }
    }
}

/** Querformat und Vollbild, solange der Player offen ist. */
@Composable
fun Fullscreen() {
    val activity = LocalContext.current as Activity
    DisposableEffect(Unit) {
        val bars = WindowCompat.getInsetsController(activity.window, activity.window.decorView)
        activity.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE
        bars.systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
        bars.hide(WindowInsetsCompat.Type.systemBars())
        onDispose {
            activity.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_UNSPECIFIED
            bars.show(WindowInsetsCompat.Type.systemBars())
        }
    }
}

@OptIn(UnstableApi::class)
@Composable
fun WatchPage(page: Page.Watch, stack: SnapshotStateList<Page>) {
    val ctx = LocalContext.current
    val player = remember { ExoPlayer.Builder(ctx).build() }
    var list by remember { mutableStateOf(page.episodes) }
    var segs by remember { mutableStateOf(listOf<Segment>()) }
    var skip by remember { mutableStateOf<Segment?>(null) }
    var error by remember { mutableStateOf<String?>(null) }
    var view by remember { mutableStateOf<PlayerView?>(null) }
    val skipFocus = remember { FocusRequester() }
    Fullscreen()

    fun save() {
        val dur = player.duration
        if (dur > 0) record(page.anime, page.number, (list?.size ?: 0).toUInt(), player.currentPosition / 1000.0, dur / 1000.0)
    }

    // Nächste Episode am Ende (wenn es sie in der gewählten Sprache gibt).
    fun next() {
        val eps = list ?: return
        val n = eps.firstOrNull { it.number > page.number } ?: return
        if (available(page.anime, n.number, page.dub)) stack[stack.lastIndex] = page.copy(number = n.number, episodes = eps)
    }

    // „Weiter“-Knopf der Leiste = nächste Episode (ExoPlayer kennt nur die eine).
    val controls = remember {
        object : ForwardingPlayer(player) {
            override fun isCommandAvailable(command: Int) = command == COMMAND_SEEK_TO_NEXT || super.isCommandAvailable(command)
            override fun getAvailableCommands(): Player.Commands =
                super.getAvailableCommands().buildUpon().add(COMMAND_SEEK_TO_NEXT).build()
            override fun seekToNext() { save(); next() }
        }
    }

    DisposableEffect(player) {
        val l = object : Player.Listener {
            override fun onPlaybackStateChanged(state: Int) {
                if (state == Player.STATE_ENDED) { save(); next() }
            }
            override fun onPlayerError(e: androidx.media3.common.PlaybackException) { error = e.message }
        }
        player.addListener(l)
        onDispose { save(); player.release() }
    }

    LaunchedEffect(page.number) {
        try {
            val eps = list ?: episodes(page.anime.id).also { list = it }
            val ep = eps.first { it.number == page.number }
            val s = stream(ep.id, page.dub)
            val http = DefaultHttpDataSource.Factory().setUserAgent(s.userAgent)
                .setDefaultRequestProperties(mapOf("Referer" to s.referer))
            val source = DefaultMediaSourceFactory(DefaultDataSource.Factory(ctx, http))
            val subs = s.subtitles.mapIndexed { i, path ->
                MediaItem.SubtitleConfiguration.Builder(Uri.fromFile(File(path)))
                    .setMimeType(MimeTypes.TEXT_VTT)
                    .setLabel(File(path).nameWithoutExtension)
                    .setSelectionFlags(if (i == 0) C.SELECTION_FLAG_DEFAULT else 0)
                    .build()
            }
            val item = MediaItem.Builder().setUri(s.variants.first().url).setMimeType(MimeTypes.APPLICATION_M3U8)
                .setSubtitleConfigurations(subs).build()
            player.setMediaSource(source.createMediaSource(item))
            resumeAt(page.anime.id, page.number)?.let { player.seekTo((it * 1000).toLong()) }
            player.setPlaybackSpeed(speed(page.anime.id).toFloat())
            player.prepare()
            player.playWhenReady = true
            s.malId?.let { segs = segments(it, page.number) }
        } catch (e: Exception) { error = message(e) }
    }

    // Wie bei Netflix: erscheint „Skip …“, bekommt es den Fokus (OK = überspringen); danach wieder das Video.
    val hasSkip = skip != null
    LaunchedEffect(hasSkip) {
        if (hasSkip) runCatching { skipFocus.requestFocus() } else view?.requestFocus()
    }

    // Skip-Knopf und Fortschritt (alle 5 s).
    LaunchedEffect(player) {
        var tick = 0
        while (true) {
            delay(500)
            val pos = player.currentPosition / 1000.0
            skip = segs.firstOrNull { pos >= it.start && pos < it.end - 0.5 }
            if (++tick % 10 == 0 && player.isPlaying) save()
        }
    }

    Box(Modifier.fillMaxSize().background(androidx.compose.ui.graphics.Color.Black)) {
        AndroidView(
            // Fokus aufs Video: Tasten der Fernbedienung blenden die Leiste ein, Pfeile wandern durch ihre Knöpfe.
            factory = {
                PlayerView(it).apply {
                    this.player = controls
                    keepScreenOn = true
                    // Fernbedienung links/rechts auf der Timeline: 10 s statt 1/20 der Länge.
                    findViewById<DefaultTimeBar>(androidx.media3.ui.R.id.exo_progress)?.setKeyTimeIncrement(10_000)
                    setShowSubtitleButton(true)
                    isFocusable = true
                    post { requestFocus() }
                    view = this
                }
            },
            modifier = Modifier.fillMaxSize(),
        )
        skip?.let { s ->
            Button(
                onClick = { player.seekTo((s.end * 1000).toLong()) },
                modifier = Modifier.align(Alignment.BottomEnd).padding(end = 24.dp, bottom = 96.dp)
                    .focusRequester(skipFocus).tvFocus(RoundedCornerShape(50)),
            ) { Text("Skip ${s.label}") }
        }
        error?.let { e -> Text(e, color = MaterialTheme.colorScheme.error, modifier = Modifier.align(Alignment.Center).padding(24.dp)) }
    }
}
