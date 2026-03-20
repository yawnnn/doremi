package android.doremi

import android.content.Context
import android.content.Intent
import android.doremi.ui.theme.DoremiTheme
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.BackHandler
import androidx.activity.compose.setContent
import androidx.compose.animation.animateContentSize
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import java.io.File
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        setContent {
            DoremiTheme {
                val doremi = remember { Doremi(this) }
                var isShareAction by remember { mutableStateOf(false) }
                val lifecycleOwner = LocalLifecycleOwner.current

                DisposableEffect(lifecycleOwner) {
                    val observer = LifecycleEventObserver { _, event ->
                        if (event == Lifecycle.Event.ON_RESUME) {
                            if (doremi.state == DoremiState.VIEW) {
                                doremi.setState(
                                    DoremiState.VIEW,
                                    view = DoremiView(Doremi.readNotes(this@MainActivity))
                                )
                            }
                        }
                    }
                    lifecycleOwner.lifecycle.addObserver(observer)
                    onDispose { lifecycleOwner.lifecycle.removeObserver(observer) }
                }

                LaunchedEffect(intent) {
                    when (intent?.action) {
                        Intent.ACTION_SEND -> {
                            when (intent.type) {
                                "text/plain" -> {
                                    intent.getStringExtra(Intent.EXTRA_TEXT)?.let {
                                        doremi.setState(
                                            DoremiState.EDIT, edit = DoremiEdit(startBody = it)
                                        )
                                    }
                                    isShareAction = true
                                }

                                else -> throw RuntimeException("Unsupported type: ${intent.type}")
                            }
                        }
                    }
                }

                val finishAction = {
                    if (isShareAction) finish()
                    else doremi.setState(
                        DoremiState.VIEW,
                        view = DoremiView(Doremi.readNotes(this))
                    )
                }

                when (doremi.state) {
                    DoremiState.VIEW -> ViewNotes(doremi)
                    DoremiState.EDIT -> doremi.edit?.let { edit ->
                        EditNote(edit = edit, onSave = { name, tags, body ->
                            val note = edit.note

                            if (note == null) {
                                Doremi.newNote(this, name, tags, body)
                            } else {
                                val update = Note(
                                    id = note.id,
                                    name = name,
                                    tags = tags,
                                    body = body,
                                    ctime = note.ctime
                                )
                                Doremi.writeNote(this, update)
                            }
                            finishAction()
                        }, onCancel = { finishAction() })
                    }
                }
            }
        }
    }
}


// TODO:
//  - autodetect already existing tags
//  - checkbox list of tags to filter in OR rather than AND
//  - normalize tags (and names?)
//  - note's priority
class Note(
    val id: String = "",
    val name: String = "",
    val tags: List<String> = emptyList(),
    val body: String = "",
    val ctime: Long = 0
) {
    fun serialize(): String = "$id\n$ctime\n$name\n${tags.joinToString(",")}\n$body"

    fun matches(filters: List<Filter>): Boolean {
        if (filters.isEmpty()) return true
        return filters.all { flt ->
            when (flt.kind) {
                FilterKind.Name -> name.contains(flt.value, ignoreCase = true)
                FilterKind.Tag -> tags.any { it.contains(flt.value, ignoreCase = true) }
                FilterKind.Body -> body.contains(flt.value, ignoreCase = true)
                FilterKind.Everything -> {
                    name.contains(flt.value, ignoreCase = true) || body.contains(
                        flt.value, ignoreCase = true
                    ) || tags.any { it.contains(flt.value, ignoreCase = true) }
                }
            }
        }
    }

    companion object {
        fun deserialize(lines: List<String>): Note {
            val id = lines.getOrNull(0) ?: ""
            val ctime = lines.getOrNull(1)?.toLongOrNull() ?: 0L
            val name = lines.getOrNull(2) ?: ""
            val tags = lines.getOrNull(3)?.split(",")?.filter { it.isNotBlank() } ?: emptyList()
            val body = lines.drop(4).joinToString("\n")
            return Note(
                id = id, name = name, tags = tags, body = body, ctime = ctime
            )
        }
    }
}

enum class FilterKind {
    Name, Tag, Body, Everything
}

data class Filter(val kind: FilterKind, val value: String)

enum class DoremiState {
    VIEW, EDIT,
}

class DoremiView(lst: List<Note>) {
    var flt by mutableStateOf("")
    val lst = mutableStateListOf<Note>().also {
        it.addAll(lst.sortedBy { note -> note.ctime })
    }

    fun parseFilters(flt: String): List<Filter> {
        val regex = Regex("""(\w:)?("([^"]+)"|(\S+))""")
        return regex.findAll(flt).map { match ->
            val kind = when (match.groups[1]?.value?.removeSuffix(":")) {
                "n" -> FilterKind.Name
                "t" -> FilterKind.Tag
                "b" -> FilterKind.Body
                else -> FilterKind.Everything
            }
            val value = match.groups[3]?.value ?: match.groups[4]?.value ?: ""
            Filter(kind, value)
        }.toList()
    }

    fun updateView(note: Note) {
        val idx = this.lst.indexOfFirst { it.id == note.id }
        when (idx) {
            -1 -> this.lst.add(note)
            else -> this.lst[idx] = note
        }
        this.lst.sortBy { it.ctime }
    }
}

data class DoremiEdit(val note: Note? = null, val startBody: String = "")

class Doremi(context: Context) {
    var state by mutableStateOf(DoremiState.VIEW)
    var view: DoremiView? by mutableStateOf(DoremiView(readNotes(context)))  // when <viewState> == LIST
    var edit: DoremiEdit? by mutableStateOf(null)                                   // when <viewState> == EDIT

    fun setState(state: DoremiState, view: DoremiView? = null, edit: DoremiEdit? = null) {
        this.state = state
        when (state) {
            DoremiState.VIEW -> this.view = view
            DoremiState.EDIT -> this.edit = edit
        }
    }

    companion object {
        fun readNotes(context: Context): List<Note> {
            val dir = File(context.filesDir, "notes")
            if (!dir.exists()) {
                dir.mkdirs()
                // seed for testing. TODO: remove
                TestData.notes.forEachIndexed { idx, note ->
                    val note = Note(
                        id = "$idx",
                        name = note.name,
                        tags = note.tags,
                        body = note.body,
                        ctime = System.currentTimeMillis() - idx * 86400000L * 15
                    )
                    val file = File(dir, "${idx}.txt")
                    file.writeText(note.serialize())
                }
            }
            return dir.listFiles()?.filter { it.extension == "txt" }?.map {
                Note.deserialize(it.readLines())
            } ?: emptyList()
        }

        fun writeNote(context: Context, note: Note) {
            val dir = File(context.filesDir, "notes")
            if (!dir.exists()) dir.mkdirs()
            val file = File(dir, "${note.id}.txt")
            file.writeText(note.serialize())
        }

        fun newNote(context: Context, name: String, tags: List<String>, body: String) {
            writeNote(
                context, Note(
                    id = "${System.currentTimeMillis()}",
                    name = name,
                    tags = tags,
                    body = body,
                    ctime = System.currentTimeMillis(),
                )
            )
        }
    }
}

@Composable
fun ViewNote(note: Note, onClick: () -> Unit) {
    // TODO:
    //  - clickable links in body + preview
    Card(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 8.dp, vertical = 4.dp)
            .clickable(role = Role.Button) { onClick() }) {
        Column(modifier = Modifier.padding(all = 8.dp)) {
            Text(
                text = note.name,
                color = MaterialTheme.colorScheme.secondary,
                style = MaterialTheme.typography.titleSmall
            )
            if (note.tags.isNotEmpty()) {
                Text(
                    text = note.tags.joinToString(", "),
                    color = MaterialTheme.colorScheme.secondary,
                    style = MaterialTheme.typography.labelMedium
                )
            }
            Text(
                text = note.body,
                modifier = Modifier
                    .animateContentSize()
                    .fillMaxWidth()
                    .padding(horizontal = 4.dp),
                style = MaterialTheme.typography.bodyMedium
            )
        }
    }
}

private fun getMonthYear(timestamp: Long): String {
    val sdf = SimpleDateFormat("MMMM yyyy", Locale.getDefault())
    return sdf.format(Date(timestamp))
}

@Composable
fun ViewNotes(doremi: Doremi) {
    val view = doremi.view ?: return
    val filters = remember(view.flt) { view.parseFilters(view.flt) }
    val notes = view.lst.filter { it.matches(filters) }

    Surface(
        modifier = Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.surface
    ) {
        // TODO:
        //  - disappear after scroll down
        Column(modifier = Modifier.safeDrawingPadding()) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(8.dp)
            ) {
                TextField(
                    value = view.flt,
                    onValueChange = { view.flt = it },
                    label = { Text("Search") },
                    modifier = Modifier.weight(1f),
                    singleLine = true,
                )
                Button(
                    onClick = {
                        doremi.setState(DoremiState.EDIT, edit = DoremiEdit())
                    }, modifier = Modifier.padding(start = 8.dp)
                ) {
                    Text("New")
                }
            }
            LazyColumn {
                itemsIndexed(notes, key = { _, note -> note.id }) { index, note ->
                    val curr = getMonthYear(note.ctime)
                    val prev = if (index > 0) getMonthYear(notes[index - 1].ctime) else null

                    if (curr != prev) {
                        Text(
                            text = curr,
                            modifier = Modifier
                                .fillMaxWidth()
                                .background(MaterialTheme.colorScheme.secondaryContainer),
                            style = MaterialTheme.typography.labelSmall.copy(
                                fontWeight = FontWeight.Bold,
                                color = MaterialTheme.colorScheme.onSecondaryContainer,
                            ),
                            textAlign = TextAlign.Center
                        )
                    }
                    ViewNote(note, onClick = {
                        doremi.setState(DoremiState.EDIT, edit = DoremiEdit(note))
                    })
                }
            }
        }
    }
}

@Composable
fun EditNote(
    edit: DoremiEdit,
    onSave: (String, List<String>, String) -> Unit,
    onCancel: () -> Unit
) {
    val note = edit.note ?: Note(body = edit.startBody)
    var name by remember(note.id, note.ctime) { mutableStateOf(note.name) }
    var tagsString by remember(note.id, note.ctime) { mutableStateOf(note.tags.joinToString(", ")) }
    var body by remember(note.id, note.ctime) { mutableStateOf(note.body) }
    val focusRequester = remember { FocusRequester() }

    BackHandler {
        onCancel()
    }

    LaunchedEffect(Unit) {
        focusRequester.requestFocus()
    }

    Surface(
        modifier = Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.surface
    ) {
        Column(
            modifier = Modifier
                .safeDrawingPadding()
                .padding(16.dp)
        ) {
            TextField(
                value = name,
                onValueChange = { name = it },
                label = { Text("Name") },
                modifier = Modifier
                    .fillMaxWidth()
                    .focusRequester(focusRequester),
                singleLine = true,
            )
            Spacer(modifier = Modifier.height(8.dp))
            TextField(
                value = tagsString,
                onValueChange = { tagsString = it },
                label = { Text("Tags") },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true,
            )
            Spacer(modifier = Modifier.height(8.dp))
            TextField(
                value = body,
                onValueChange = { body = it },
                label = { Text("Body") },
                modifier = Modifier
                    .fillMaxWidth()
                    .weight(1f)
            )
            Spacer(modifier = Modifier.height(16.dp))
            Row {
                Button(onClick = onCancel) {
                    Text("Cancel")
                }
                Spacer(modifier = Modifier.weight(1f))
                Button(onClick = {
                    val tags = tagsString.split(",").map { it.trim() }.filter { it.isNotEmpty() }
                    onSave(name, tags, body)
                }) {
                    Text("Save")
                }
            }
        }
    }
}