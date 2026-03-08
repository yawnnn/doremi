package android.doremi

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
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import java.io.File
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            DoremiTheme {
                val context = this
                val viewModel = remember {
                    val initialNotes = loadNotes(context)
                    NotesViewModel(initialNotes)
                }

                if (viewModel.editingNote != null) {
                    EditNote(
                        note = viewModel.editingNote!!,
                        onSave = { updatedNote ->
                            viewModel.saveUpdatedNote(context, updatedNote)
                            viewModel.editingNote = null
                        },
                        onCancel = {
                            viewModel.editingNote = null
                        }
                    )
                } else {
                    ViewNotes(viewModel)
                }
            }
        }
    }
}

private fun Note.toFileContent(): String = "$ctime\n$name\n${tags.joinToString(",")}\n$body"

private fun File.toNote(): Note {
    val lines = readLines()
    val ctime = lines.getOrNull(0)?.toLongOrNull() ?: 0L
    val name = lines.getOrNull(1) ?: ""
    val tags = lines.getOrNull(2)?.split(",")?.filter { it.isNotBlank() } ?: emptyList()
    val body = lines.drop(3).joinToString("\n")
    return Note(id = nameWithoutExtension, name = name, tags = tags, body = body, ctime = ctime)
}

private fun loadNotes(context: android.content.Context): List<Note> {
    val dir = File(context.filesDir, "notes")
    if (!dir.exists()) {
        dir.mkdirs()
        // seed the first time
        TestData.notes.forEachIndexed { idx, testNote ->
            val note = Note(
                id = "note_$idx",
                name = testNote.name,
                tags = testNote.tags,
                body = testNote.body,
                ctime = System.currentTimeMillis() - idx * 86400000L * 15
            )
            val file = File(dir, "${note.id}.txt")
            file.writeText(note.toFileContent())
        }
    }
    return dir.listFiles()?.filter { it.extension == "txt" }?.map { it.toNote() } ?: emptyList()
}

private fun saveNote(context: android.content.Context, note: Note) {
    val dir = File(context.filesDir, "notes")
    if (!dir.exists()) dir.mkdirs()
    val file = File(dir, "${note.id}.txt")
    file.writeText(note.toFileContent())
}

// TODO:
//  - autodetect already existing tags
//  - checkbox list of tags to filter in OR rather than AND
//  - normalize tags (and names?)
//  - note's priority
data class Note(
    val id: String,
    val name: String,
    val tags: List<String>,
    val body: String,
    val ctime: Long = 0
)

class NotesViewModel(
    notes: List<Note>
) : ViewModel() {
    val notes = mutableStateListOf<Note>().also {
        it.addAll(notes.sortedBy { n -> n.ctime })
    }
    var filter by mutableStateOf("")  // eg: one_word_match "two-word match" n:name t:"two-word tag" b:"three-word body content"
    var editingNote by mutableStateOf<Note?>(null)

    fun saveUpdatedNote(context: android.content.Context, updatedNote: Note) {
        val index = this@NotesViewModel.notes.indexOfFirst { it.id == updatedNote.id }
        if (index != -1) {
            this@NotesViewModel.notes[index] = updatedNote
        } else {
            this@NotesViewModel.notes.add(updatedNote)
        }
        this@NotesViewModel.notes.sortedBy { it.ctime }
        saveNote(context, updatedNote)
    }
}

@Composable
fun ViewNote(note: Note, onClick: () -> Unit) {
    // TODO:
    //  - prettier (everything)
    //  - clickable links in body
    Card(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 8.dp, vertical = 4.dp)
            .clickable { onClick() }
    ) {
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
                    style = MaterialTheme.typography.labelSmall
                )
            }
            Surface(
                shape = MaterialTheme.shapes.medium,
                color = MaterialTheme.colorScheme.surfaceVariant,
                modifier = Modifier
                    .animateContentSize()
                    .fillMaxWidth()
            ) {
                Text(
                    text = note.body,
                    modifier = Modifier.padding(horizontal = 4.dp),
                    style = MaterialTheme.typography.bodyMedium
                )
            }
        }
    }
}

private data class Filter(val kind: String?, val value: String)

private fun parseFilters(query: String): List<Filter> {
    val regex = Regex("""(\w:)?("([^"]+)"|(\S+))""")
    return regex.findAll(query).map { match ->
        val kind = match.groups[1]?.value?.removeSuffix(":")
        val value = match.groups[3]?.value ?: match.groups[4]?.value ?: ""
        Filter(kind, value)
    }.toList()
}

private fun Note.matches(filters: List<Filter>): Boolean {
    if (filters.isEmpty()) return true
    return filters.all { c ->
        when (c.kind) {
            "n" -> name.contains(c.value, ignoreCase = true)
            "t" -> tags.any { it.contains(c.value, ignoreCase = true) }
            "b" -> body.contains(c.value, ignoreCase = true)
            else -> {
                name.contains(c.value, ignoreCase = true) ||
                        body.contains(c.value, ignoreCase = true) ||
                        tags.any { it.contains(c.value, ignoreCase = true) }
            }
        }
    }
}

private fun getMonthYear(timestamp: Long): String {
    val sdf = SimpleDateFormat("MMMM yyyy", Locale.getDefault())
    return sdf.format(Date(timestamp))
}

@Composable
fun ViewNotes(vm: NotesViewModel) {
    val filters = remember(vm.filter) { parseFilters(vm.filter) }
    val notes = remember(vm.notes, filters) {
        vm.notes.filter { it.matches(filters) }
    }

    Surface(modifier = Modifier.fillMaxSize()) {
        // TODO:
        //  - disappear after scroll down
        Column {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(8.dp)
            ) {
                TextField(
                    value = vm.filter,
                    onValueChange = { vm.filter = it },
                    label = { Text("Search") },
                    modifier = Modifier.weight(1f),
                    singleLine = true,
                )
                Button(
                    onClick = {
                        vm.editingNote = Note(
                            id = "note_${System.currentTimeMillis()}",
                            name = "",
                            tags = emptyList(),
                            body = "",
                            ctime = System.currentTimeMillis()
                        )
                    },
                    modifier = Modifier.padding(start = 8.dp)
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
                    ViewNote(note, onClick = { vm.editingNote = note })
                }
            }
        }
    }
}

@Composable
fun EditNote(note: Note, onSave: (Note) -> Unit, onCancel: () -> Unit) {
    var name by remember { mutableStateOf(note.name) }
    var tagsString by remember { mutableStateOf(note.tags.joinToString(", ")) }
    var body by remember { mutableStateOf(note.body) }

    BackHandler {
        onCancel()
    }

    Surface(modifier = Modifier.fillMaxSize()) {
        Column(modifier = Modifier.padding(16.dp)) {
            TextField(
                value = name,
                onValueChange = { name = it },
                label = { Text("Name") },
                modifier = Modifier.fillMaxWidth(),
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
                    onSave(note.copy(name = name, tags = tags, body = body))
                }) {
                    Text("Save")
                }
            }
        }
    }
}
