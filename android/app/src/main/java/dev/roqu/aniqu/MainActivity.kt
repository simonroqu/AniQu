// AniQu für Android: Seiten der App in Compose; Bausteine (Bubble, Suche, Raster, Player, Theme) aus qu-core
// (`dev.roqu.qu`, android/src in qu-core), Daten aus Rust (aniqu-core über aniqu-ffi, `uniffi.aniqu_ffi`).
// Aussehen und Bedienung wie die Desktop-App; alles per Fernbedienung erreichbar (Android TV).
package dev.roqu.aniqu

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.runtime.*
import androidx.compose.ui.unit.dp
import dev.roqu.qu.*
import kotlinx.coroutines.async
import kotlinx.coroutines.coroutineScope
import uniffi.aniqu_ffi.*
import java.io.File

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        init(filesDir.path, cacheDir.path)
        enableEdgeToEdge()
        val link = intent?.dataString
        setContent { QuTheme { App(link) } }
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

@Composable
fun App(link: String?) {
    val nav = remember { Nav<Page>(Page.Home) }
    LaunchedEffect(link) {
        val ids = link?.let { parseLink(it) } ?: return@LaunchedEffect
        nav.open(Page.Show(bare(ids[0])))
    }
    QuApp(
        nav,
        placeholder = "Search anime",
        searches = ::searches,
        onSearch = { addSearch(it); nav.open(Page.Results(it)) },
        bubble = { it !is Page.Watch },
    ) { page ->
        when (page) {
            Page.Home -> HomePage(
                open = { nav.open(Page.Show(it)) },
                resume = { nav.open(Page.Watch(it.anime, it.episode, dub(it.anime.id), null)) },
            )
            is Page.Results -> ResultsPage(page.query) { nav.open(Page.Show(it)) }
            is Page.Show -> ShowPage(page.anime) { nav.open(it) }
            is Page.Watch -> WatchPage(page, nav)
        }
    }
}

@Composable
fun AnimeCard(a: Anime, label: String? = null, fraction: Double? = null, open: () -> Unit) =
    PosterCard(a.title, a.poster, label = label, fraction = fraction, onClick = open)

@Composable
fun HomePage(open: (Anime) -> Unit, resume: (Resume) -> Unit) {
    var popular by remember { mutableStateOf(savedTrending()) }
    var error by remember { mutableStateOf<String?>(null) }
    val going = remember { continueWatching(12u) }
    val saved = remember { watchlist() }
    LaunchedEffect(Unit) {
        try { popular = trending() } catch (e: Exception) { error = message(e) }
    }
    Grid {
        var first = true
        fun top() = (if (first) 0 else 20).also { first = false }
        if (going.isNotEmpty()) {
            header("Continue watching", top().dp)
            items(going.size, key = { "c" + going[it].anime.id }) { i ->
                val r = going[i]
                AnimeCard(r.anime, "Episode ${r.episode}", r.fraction) { resume(r) }
            }
        }
        if (saved.isNotEmpty()) {
            header("Watchlist", top().dp)
            items(saved.size, key = { "w" + saved[it].id }) { AnimeCard(saved[it]) { open(saved[it]) } }
        }
        header("Trending", top().dp)
        error?.let { note(it, Red) }
        val ids = saved.map { it.id }.toSet()
        val rest = popular.filter { it.id !in ids }
        if (popular.isEmpty() && error == null) ghosts(12)
        items(rest.size, key = { "t" + rest[it].id }) { AnimeCard(rest[it]) { open(rest[it]) } }
    }
}

@Composable
fun ResultsPage(query: String, open: (Anime) -> Unit) {
    var found by remember { mutableStateOf<List<Anime>?>(null) }
    var error by remember { mutableStateOf<String?>(null) }
    LaunchedEffect(query) {
        try { found = search(query) } catch (e: Exception) { error = message(e) }
    }
    Grid {
        header(query, 0.dp)
        error?.let { note(it, Red) }
        val list = found
        when {
            list == null && error == null -> ghosts(12)
            list != null && list.isEmpty() -> note("Nothing found")
            list != null -> items(list.size, key = { list[it].id }) { AnimeCard(list[it]) { open(list[it]) } }
        }
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
    Grid(minWidth = 170.dp) {
        wide {
            DetailHeader(
                anime.title,
                anime.poster,
                meta = info?.let { (it.info + it.genres).joinToString(" · ") },
                description = info?.description,
            ) {
                TabButton("Sub", !isDub) { isDub = false; setDub(start.id, false) }
                TabButton("Dub", isDub, enabled = anime.dub != null) { isDub = true; setDub(start.id, true) }
                IconButton(
                    if (listed) QuIcons.watchlisted else QuIcons.watchlist,
                    if (listed) "Remove from watchlist" else "Add to watchlist",
                    tint = if (listed) Accent else Fg,
                ) { listed = toggleWatchlist(anime) }
            }
        }
        error?.let { note(it, Red) }
        val eps = list
        if (eps == null && error == null) ghostTiles(12)
        if (eps != null && eps.isEmpty()) note("No episodes yet")
        if (eps != null) {
            items(eps.size, key = { eps[it].id }) { i ->
                val ep = eps[i]
                val seen = remember(ep.id) { watched(anime.id, ep.number) }
                Tile("Episode ${ep.number}", ep.title, seen?.fraction, seen?.finished == true, available(anime, ep.number, isDub)) {
                    play(Page.Watch(anime, ep.number, isDub, eps))
                }
            }
        }
    }
}

@Composable
fun WatchPage(page: Page.Watch, nav: Nav<Page>) {
    var list by remember { mutableStateOf(page.episodes) }
    var malId by remember { mutableStateOf<String?>(null) }
    // Nächste Episode (wenn es sie in der gewählten Sprache gibt).
    val following = list?.firstOrNull { it.number > page.number }?.takeIf { available(page.anime, it.number, page.dub) }
    VideoPlayer(
        load = {
            val eps = list ?: episodes(page.anime.id).also { list = it }
            val s = stream(eps.first { it.number == page.number }.id, page.dub)
            malId = s.malId
            Video(
                url = s.variants.first().url,
                mime = androidx.media3.common.MimeTypes.APPLICATION_M3U8,
                headers = mapOf("Referer" to s.referer),
                userAgent = s.userAgent,
                subtitles = s.subtitles.map { Subtitle(it, File(it).nameWithoutExtension) },
                start = resumeAt(page.anime.id, page.number),
                speed = speed(page.anime.id).toFloat(),
            )
        },
        message = ::message,
        save = { pos, dur -> record(page.anime, page.number, (list?.size ?: 0).toUInt(), pos, dur) },
        skips = { malId?.let { id -> segments(id, page.number).map { Skip(it.start, it.end, it.label) } } ?: emptyList() },
        next = following?.let { n -> { nav.replace(page.copy(number = n.number, episodes = list)) } },
    )
}
