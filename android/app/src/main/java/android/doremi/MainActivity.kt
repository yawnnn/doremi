package android.doremi

import android.doremi.ui.theme.DoremiTheme
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.animateContentSize
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Card
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import java.io.File

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            DoremiTheme() {
                val viewModel = remember {
                    NotesViewModel(loadNotes(this))
                }
                ViewNotes(viewModel)

                loadNotes(this).forEach {
                    saveNote(this, it)
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
                ctime = System.currentTimeMillis()
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

data class Note(
    val id: String,
    val name: String,
    val tags: List<String>,
    val body: String,
    val ctime: Long = 0
)

class NotesViewModel(
    val notes: List<Note>
) : ViewModel() {
    // var notes = mutableStateListOf<Note>().also { it.addAll(notes) }

    // eg: one_word_match "two-word match" n:name t:"two-word tag" b:"three-word body content"
    var filter by mutableStateOf("")
}

@Composable
fun ViewNote(note: Note) {
    // TODO:
    //  - prettier (everything)
    //  - editing inline
    //  - clickable links in body
    Card(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 8.dp, vertical = 4.dp)
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
            //Spacer(modifier = Modifier.height(4.dp))
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
                    //maxLines = if (isOpen) Int.MAX_VALUE else 1,
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

@Composable
fun ViewNotes(vm: NotesViewModel) {
    val filters = remember(vm.filter) { parseFilters(vm.filter) }
    val notes = remember(vm.notes, filters) { vm.notes.filter { it.matches(filters) } }

    Surface(modifier = Modifier.fillMaxSize()) {
        Column() {
            // TODO:
            //  - disappear after scroll down
            TextField(
                value = vm.filter,
                onValueChange = { vm.filter = it },
                label = { Text("Search") },
                modifier = Modifier.fillMaxWidth()
            )
            // TODO:
            //  - group by month/show month headers
            LazyColumn() {
                items(notes) { note ->
                    ViewNote(note)
                }
            }
        }
    }
}