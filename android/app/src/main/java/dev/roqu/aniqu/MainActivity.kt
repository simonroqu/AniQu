// AniQu für Android: Oberfläche in Compose, alles andere (hianime, Fortschritt, Listen, AniSkip) kommt aus
// Rust (aniqu-core über aniqu-ffi, Kotlin-Anbindung in uniffi.aniqu_ffi, erzeugt von build-rust.sh).
// Aussehen wie die Desktop-App (qu-core): Theme „Last Horizon“, schwebende Nav-Bubble oben links, Suche wie Spotlight.
// Texte in der Oberfläche: Englisch. Alles muss per Fernbedienung erreichbar sein (Android TV, `tvFocus`).
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
import androidx.compose.foundation.focusGroup
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyGridScope
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.runtime.snapshots.SnapshotStateList
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.focus.FocusDirection
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.compose.ui.window.DialogWindowProvider
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
import uniffi.aniqu_ffi.*
import java.io.File

// Farben wie qu-core ohne Omarchy (theme.rs `DEFAULT_COLORS`, „Last Horizon“) und die Glas-Bubbles aus style.css.
val Bg = Color(0xFF0C0B0C)
val Fg = Color(0xFFFAFCFB)
val Accent = Color(0xFFB59790)
val Red = Color(0xFFC38B7B)
val Dim = Fg.copy(alpha = 0.65f)
val Tile = Fg.copy(alpha = 0.06f)
val Glass = Color(0x8C18181C)
val GlassBorder = Color(0x1FFFFFFF)
val SearchCard = Color(0xEB1E1E22)

val Scheme = darkColorScheme(
    primary = Accent, onPrimary = Bg, background = Bg, onBackground = Fg, surface = Bg, onSurface = Fg,
    surfaceVariant = Tile, onSurfaceVariant = Dim, error = Red,
)

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        init(filesDir.path, cacheDir.path)
        enableEdgeToEdge()
        val link = intent?.dataString
        setContent {
            MaterialTheme(colorScheme = Scheme) {
                Surface(Modifier.fillMaxSize(), color = Bg) { App(link) }
            }
        }
    }
}

sealed interface Page {
    data object Home : Page
    data class Results(val query: String) : Page
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

/** Dunkles Glas wie `.bubble` (qu-core style.css). */
fun Modifier.glass(shape: Shape = CircleShape) =
    shadow(12.dp, shape).background(Glass, shape).border(1.dp, GlassBorder, shape)

/** Runder Icon-Knopf in einer Bubble (Icons aus qu-core, `android/icons.py`). */
@Composable
fun BubbleButton(icon: Int, label: String, onClick: () -> Unit) {
    Box(
        Modifier.size(40.dp).clip(CircleShape).tvFocus(CircleShape, 1.1f).clickable(onClickLabel = label, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) { Icon(painterResource(icon), contentDescription = label, tint = Color.White, modifier = Modifier.size(18.dp)) }
}

@Composable
fun App(link: String?) {
    val stack = remember { mutableStateListOf<Page>(Page.Home) }
    var searching by remember { mutableStateOf(false) }
    LaunchedEffect(link) {
        val ids = link?.let { parseLink(it) } ?: return@LaunchedEffect
        stack.add(Page.Show(bare(ids[0])))
    }
    BackHandler(stack.size > 1) { stack.removeAt(stack.lastIndex) }
    val page = stack.last()
    val content = remember { FocusRequester() }
    Box(Modifier.fillMaxSize()) {
        // key: jede Seite frisch (eigener Zustand, Player wird beim Verlassen freigegeben).
        key(page) { Box(Modifier.focusRequester(content).focusGroup()) {
            when (page) {
                Page.Home -> HomePage(
                    open = { stack.add(Page.Show(it)) },
                    resume = { stack.add(Page.Watch(it.anime, it.episode, dub(it.anime.id), null)) },
                )
                is Page.Results -> ResultsPage(page.query) { stack.add(Page.Show(it)) }
                is Page.Show -> ShowPage(page.anime) { stack.add(it) }
                is Page.Watch -> WatchPage(page, stack)
            }
        } }
        // Schwebende Navigation oben links wie am Desktop (`[nav] top` in qu.toml: back, home, search).
        if (page !is Page.Watch) {
            // Runter: die schwebende Bubble liegt über der Seite, Compose findet von hier aus keinen Weg hinein.
            Row(
                Modifier.safeDrawingPadding().padding(12.dp).glass().padding(3.dp).onPreviewKeyEvent {
                    val down = it.type == KeyEventType.KeyDown && it.key == Key.DirectionDown
                    down && content.requestFocus(FocusDirection.Down)
                },
            ) {
                if (stack.size > 1) BubbleButton(R.drawable.qu_back, "Back") { stack.removeAt(stack.lastIndex) }
                BubbleButton(R.drawable.qu_home, "Home") { stack.removeRange(1, stack.size) }
                BubbleButton(R.drawable.qu_search, "Search") { searching = true }
            }
        }
    }
    if (searching) {
        SearchModal(close = { searching = false }) { q ->
            searching = false
            addSearch(q)
            stack.add(Page.Results(q))
        }
    }
}

/** Suchfenster wie Spotlight (qu-core `search.rs`): abgedunkelt, Karte oben in der Mitte, letzte Suchen darunter. */
@Composable
fun SearchModal(close: () -> Unit, submit: (String) -> Unit) {
    var query by remember { mutableStateOf("") }
    val recent = remember { searches() }
    val field = remember { FocusRequester() }
    LaunchedEffect(Unit) { field.requestFocus() }
    Dialog(onDismissRequest = close, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        // Abdunkeln macht die Box unten (wie am Desktop 45 %), nicht zusätzlich Android.
        (LocalView.current.parent as? DialogWindowProvider)?.window?.setDimAmount(0f)
        val focus = LocalFocusManager.current
        Box(
            Modifier.fillMaxSize().background(Color(0x73000000))
                .clickable(interactionSource = remember { MutableInteractionSource() }, indication = null, onClick = close),
            contentAlignment = Alignment.TopCenter,
        ) {
            val shape = RoundedCornerShape(18.dp)
            Column(
                Modifier.padding(top = 72.dp, start = 16.dp, end = 16.dp).widthIn(max = 560.dp).fillMaxWidth()
                    .shadow(30.dp, shape).background(SearchCard, shape).border(1.dp, GlassBorder, shape)
                    .clickable(interactionSource = remember { MutableInteractionSource() }, indication = null) {}
                    .padding(8.dp),
            ) {
                Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(horizontal = 8.dp)) {
                    Icon(painterResource(R.drawable.qu_search), null, tint = Dim, modifier = Modifier.size(18.dp))
                    BasicTextField(
                        value = query,
                        onValueChange = { query = it },
                        singleLine = true,
                        textStyle = TextStyle(color = Color.White, fontSize = 19.sp),
                        cursorBrush = SolidColor(Accent),
                        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                        keyboardActions = KeyboardActions(onSearch = { if (query.isNotBlank()) submit(query.trim()) }),
                        // Fernbedienung: Runter verlässt das Feld zu den letzten Suchen (im Textfeld bewegt es sonst nur den Cursor).
                        modifier = Modifier.weight(1f).padding(start = 12.dp).heightIn(min = 44.dp).focusRequester(field)
                            .onPreviewKeyEvent {
                                val down = it.type == KeyEventType.KeyDown && it.key == Key.DirectionDown
                                down && focus.moveFocus(FocusDirection.Down)
                            },
                        decorationBox = { inner ->
                            Box(contentAlignment = Alignment.CenterStart) {
                                if (query.isEmpty()) Text("Search anime", color = Dim, fontSize = 19.sp)
                                inner()
                            }
                        },
                    )
                }
                // Leeres Feld = letzte Suchen, sonst die passenden davon (`[search] suggestions = "history"`).
                val shown = recent.filter { it.contains(query.trim(), ignoreCase = true) }
                shown.forEach { s ->
                    Text(
                        s,
                        color = Color.White,
                        modifier = Modifier.fillMaxWidth().clip(RoundedCornerShape(12.dp)).tvFocus(RoundedCornerShape(12.dp), 1f)
                            .clickable { submit(s) }.padding(horizontal = 12.dp, vertical = 10.dp),
                    )
                }
            }
        }
    }
}

/** Platz unter der Nav-Bubble, damit sie nichts verdeckt. */
val TopSpace = 64.dp

@Composable
fun PosterCard(a: Anime, label: String? = null, fraction: Double? = null, onClick: () -> Unit) {
    Column(Modifier.tvFocus().clip(RoundedCornerShape(10.dp)).clickable(onClick = onClick).padding(4.dp)) {
        Box {
            AsyncImage(
                model = a.poster,
                contentDescription = a.title,
                contentScale = ContentScale.Crop,
                modifier = Modifier.fillMaxWidth().aspectRatio(2f / 3f).clip(RoundedCornerShape(8.dp)).background(Tile),
            )
            if (fraction != null) {
                LinearProgressIndicator(
                    progress = { fraction.toFloat() },
                    color = Accent,
                    trackColor = Color(0x66000000),
                    modifier = Modifier.fillMaxWidth().align(Alignment.BottomCenter).padding(6.dp).clip(CircleShape),
                )
            }
        }
        Text(a.title, maxLines = 2, overflow = TextOverflow.Ellipsis, fontWeight = FontWeight.Bold, fontSize = 15.sp,
            color = Color.White, modifier = Modifier.padding(top = 6.dp))
        if (label != null) Text(label, fontSize = 13.sp, color = Dim)
    }
}

/** Graue Platzhalter in Form der Karten, solange nichts da ist (wie `.skeleton` am Desktop). */
fun LazyGridScope.ghosts(n: Int) {
    items(n) {
        Column(Modifier.padding(4.dp)) {
            Box(Modifier.fillMaxWidth().aspectRatio(2f / 3f).clip(RoundedCornerShape(8.dp)).background(Tile))
            Box(Modifier.padding(top = 8.dp).fillMaxWidth(0.7f).height(14.dp).clip(RoundedCornerShape(4.dp)).background(Tile))
        }
    }
}

fun LazyGridScope.header(text: String, top: androidx.compose.ui.unit.Dp = 20.dp) {
    item(span = { GridItemSpan(maxLineSpan) }) {
        Text(text, fontWeight = FontWeight.Bold, fontSize = 24.sp, color = Color.White, modifier = Modifier.padding(top = top, bottom = 4.dp))
    }
}

fun LazyGridScope.note(text: String, color: Color = Dim) {
    item(span = { GridItemSpan(maxLineSpan) }) { Text(text, color = color, modifier = Modifier.padding(vertical = 8.dp)) }
}

@Composable
fun PosterGrid(content: LazyGridScope.() -> Unit) {
    LazyVerticalGrid(
        columns = GridCells.Adaptive(120.dp),
        contentPadding = PaddingValues(start = 12.dp, end = 12.dp, top = TopSpace, bottom = 24.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
        modifier = Modifier.safeDrawingPadding(),
        content = content,
    )
}

@Composable
fun HomePage(open: (Anime) -> Unit, resume: (Resume) -> Unit) {
    var popular by remember { mutableStateOf(savedTrending()) }
    var error by remember { mutableStateOf<String?>(null) }
    val going = remember { continueWatching(12u) }
    val saved = remember { watchlist() }
    LaunchedEffect(Unit) {
        try { popular = trending() } catch (e: Exception) { error = message(e) }
    }
    PosterGrid {
        var first = true
        fun top() = (if (first) 0.dp else 20.dp).also { first = false }
        if (going.isNotEmpty()) {
            header("Continue watching", top())
            items(going, key = { "c" + it.anime.id }) { r -> PosterCard(r.anime, "Episode ${r.episode}", r.fraction) { resume(r) } }
        }
        if (saved.isNotEmpty()) {
            header("Watchlist", top())
            items(saved, key = { "w" + it.id }) { PosterCard(it) { open(it) } }
        }
        header("Trending", top())
        error?.let { note(it, Red) }
        val ids = saved.map { it.id }.toSet()
        if (popular.isEmpty() && error == null) ghosts(12)
        items(popular.filter { it.id !in ids }, key = { "t" + it.id }) { PosterCard(it) { open(it) } }
    }
}

@Composable
fun ResultsPage(query: String, open: (Anime) -> Unit) {
    var found by remember { mutableStateOf<List<Anime>?>(null) }
    var error by remember { mutableStateOf<String?>(null) }
    LaunchedEffect(query) {
        try { found = search(query) } catch (e: Exception) { error = message(e) }
    }
    PosterGrid {
        header(query, 0.dp)
        error?.let { note(it, Red) }
        val list = found
        when {
            list == null && error == null -> ghosts(12)
            list != null && list.isEmpty() -> note("Nothing found")
            list != null -> items(list, key = { it.id }) { PosterCard(it) { open(it) } }
        }
    }
}

/** Umschalter Sub/Dub wie `.tabs` am Desktop: gewählt = Akzentfarbe mit Strich darunter. */
@Composable
fun TabButton(text: String, selected: Boolean, enabled: Boolean = true, onClick: () -> Unit) {
    Column(
        Modifier.alpha(if (enabled) 1f else 0.4f).clip(RoundedCornerShape(8.dp)).tvFocus(RoundedCornerShape(8.dp))
            .clickable(enabled = enabled, onClick = onClick).padding(horizontal = 14.dp, vertical = 8.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text(text, color = if (selected) Accent else Fg, fontWeight = FontWeight.Bold)
        Box(Modifier.padding(top = 4.dp).width(28.dp).height(2.dp).background(if (selected) Accent else Color.Transparent))
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
    LazyVerticalGrid(
        columns = GridCells.Adaptive(170.dp),
        contentPadding = PaddingValues(start = 16.dp, end = 16.dp, top = TopSpace, bottom = 24.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
        modifier = Modifier.safeDrawingPadding(),
    ) {
        item(span = { GridItemSpan(maxLineSpan) }) {
            Row(horizontalArrangement = Arrangement.spacedBy(20.dp), modifier = Modifier.padding(bottom = 12.dp)) {
                AsyncImage(
                    model = anime.poster,
                    contentDescription = null,
                    contentScale = ContentScale.Crop,
                    modifier = Modifier.width(140.dp).aspectRatio(2f / 3f).clip(RoundedCornerShape(10.dp)).background(Tile),
                )
                Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    Text(anime.title, fontWeight = FontWeight.Bold, fontSize = 28.sp, lineHeight = 32.sp, color = Color.White)
                    info?.let { Text((it.info + it.genres).joinToString(" · "), color = Dim, fontSize = 14.sp) }
                    info?.description?.takeIf { it.isNotEmpty() }?.let { d ->
                        Text(d, maxLines = if (more) Int.MAX_VALUE else 4, overflow = TextOverflow.Ellipsis, fontSize = 15.sp)
                        TextButton(onClick = { more = !more }, modifier = Modifier.tvFocus(RoundedCornerShape(50))) {
                            Text(if (more) "Show less" else "Show more", color = Fg)
                        }
                    }
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                        TabButton("Sub", !isDub) { isDub = false; setDub(start.id, false) }
                        TabButton("Dub", isDub, enabled = anime.dub != null) { isDub = true; setDub(start.id, true) }
                        val label = if (listed) "Remove from watchlist" else "Add to watchlist"
                        Box(
                            Modifier.size(40.dp).clip(CircleShape).tvFocus(CircleShape, 1.1f)
                                .clickable(onClickLabel = label) { listed = toggleWatchlist(anime) },
                            contentAlignment = Alignment.Center,
                        ) {
                            Icon(painterResource(if (listed) R.drawable.qu_watchlisted else R.drawable.qu_watchlist), label,
                                tint = if (listed) Accent else Fg, modifier = Modifier.size(20.dp))
                        }
                    }
                }
            }
        }
        error?.let { note(it, Red) }
        val eps = list
        if (eps == null && error == null) {
            items(12) { Box(Modifier.height(66.dp).clip(RoundedCornerShape(12.dp)).background(Tile)) }
        }
        if (eps != null && eps.isEmpty()) note("No episodes yet")
        if (eps != null) {
            items(eps, key = { it.id }) { ep -> EpisodeTile(anime, ep, isDub) { play(Page.Watch(anime, ep.number, isDub, eps)) } }
        }
    }
}

/** Episoden-Kachel wie `.episode` am Desktop: Nummer, Titel, Fortschritt; gesehen = halb durchsichtig. */
@Composable
fun EpisodeTile(anime: Anime, ep: Episode, isDub: Boolean, onClick: () -> Unit) {
    val ok = available(anime, ep.number, isDub)
    val seen = remember(ep.id) { watched(anime.id, ep.number) }
    val shape = RoundedCornerShape(12.dp)
    Column(
        Modifier.alpha(if (!ok) 0.35f else if (seen?.finished == true) 0.5f else 1f).tvFocus(shape)
            .clip(shape).background(Tile).clickable(enabled = ok, onClick = onClick).padding(horizontal = 12.dp, vertical = 10.dp),
    ) {
        Text("Episode ${ep.number}", fontWeight = FontWeight.Bold, color = Color.White)
        Text(ep.title, color = Dim, fontSize = 13.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
        LinearProgressIndicator(
            progress = { seen?.fraction?.toFloat() ?: 0f },
            color = Accent,
            trackColor = if (seen != null) Fg.copy(alpha = 0.15f) else Color.Transparent,
            modifier = Modifier.fillMaxWidth().padding(top = 6.dp).height(3.dp).clip(CircleShape),
        )
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

    Box(Modifier.fillMaxSize().background(Color.Black)) {
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
            // Bubble wie am Desktop („Skip intro“ rechts über der Timeline).
            Text(
                "Skip ${s.label}",
                color = Color.White,
                fontWeight = FontWeight.Bold,
                modifier = Modifier.align(Alignment.BottomEnd).padding(end = 24.dp, bottom = 96.dp)
                    .focusRequester(skipFocus).tvFocus(CircleShape).glass().clip(CircleShape)
                    .clickable { player.seekTo((s.end * 1000).toLong()) }.padding(horizontal = 18.dp, vertical = 10.dp),
            )
        }
        error?.let { e -> Text(e, color = MaterialTheme.colorScheme.error, modifier = Modifier.align(Alignment.Center).padding(24.dp)) }
    }
}
