console.log('app.js: Top of file');

document.addEventListener('alpine:init', () => {
    console.log('app.js: alpine:init fired');

    try {
        Alpine.store('themeSwitcherStore', {
            theme: 'light', // Minimal default
            initStore() { console.log('themeSwitcherStore.initStore() called'); document.documentElement.setAttribute('data-bs-theme', this.theme); },
            applyTheme() { console.log('themeSwitcherStore.applyTheme() called'); document.documentElement.setAttribute('data-bs-theme', this.theme); localStorage.setItem('theme', this.theme); },
            toggleTheme() { 
                this.theme = this.theme === 'light' ? 'dark' : 'light'; 
                this.applyTheme(); 
                console.log('themeSwitcherStore.toggleTheme() to:', this.theme);
            }
        });
        // Initialize immediately, but ensure Alpine is ready for setAttribute on <html>
        // Deferring this specific init call slightly can sometimes help if there are race conditions with <html> attributes.
        Alpine.nextTick(() => Alpine.store('themeSwitcherStore').initStore());
        console.log('app.js: themeSwitcherStore registered');
    } catch (e) {
        console.error('Error registering themeSwitcherStore:', e);
    }

    try {
        Alpine.store('uiStore', {
            isLoading: false
        });
        console.log('app.js: uiStore registered');
    } catch (e) {
        console.error('Error registering uiStore:', e);
    }

    try {
        Alpine.data('todoApp', () => {
            console.log('app.js: Alpine.data for todoApp invoked');
            return {
                // Data properties
                allTodos: [], // Complete list of all todos
                todos: [],    // Filtered list for display
                total_todos: 0,
                completed_todos: 0,
                pending_todos: 0,
                newTodo: '',
                activeFilter: 'all',
                errorMessage: null,
                errorDetails: null,
                // Pagination
                currentPage: 1,
                itemsPerPage: 50,
                totalPages: 1,
                totalItems: 0,
                // showLoadingState is via $store.uiStore.isLoading
                
                // Methods
                init() { 
                    console.log('todoApp init() - minimal'); 
                    this.activeFilter = new URLSearchParams(window.location.search).get('completed') === 'true' ? 'completed' : (new URLSearchParams(window.location.search).get('completed') === 'false' ? 'active' : 'all');
                    
                    // Check for page parameter in URL
                    const pageParam = new URLSearchParams(window.location.search).get('page');
                    if (pageParam && !isNaN(parseInt(pageParam))) {
                        this.currentPage = parseInt(pageParam);
                    }
                    
                    this.fetchInitialTodos();
                },
                
                // Apply the current filter to the todos list
                applyFilter() {
                    if (this.activeFilter === 'all') {
                        // No filter, show all todos
                        this.todos = [...this.allTodos];
                    } else {
                        // Apply filter
                        const isCompleted = this.activeFilter === 'completed';
                        this.todos = this.allTodos.filter(t => t.completed === isCompleted);
                    }
                    console.log(`Applied filter '${this.activeFilter}': showing ${this.todos.length} of ${this.allTodos.length} todos`);
                },
                
                // Fetch todos from backend API with pagination
                fetchInitialTodos() {
                    console.log('fetchInitialTodos called');
                    
                    // Show loading state
                    Alpine.store('uiStore').isLoading = true;
                    
                    // Build URL with pagination parameters
                    const limit = this.itemsPerPage;
                    const offset = (this.currentPage - 1) * this.itemsPerPage;
                    let url = `/todos?limit=${limit}&offset=${offset}`;
                    
                    // Add filter parameter if needed
                    if (this.activeFilter !== 'all') {
                        url += `&completed=${this.activeFilter === 'completed'}`;
                    }
                    
                    console.log(`Fetching todos with URL: ${url}`);
                    
                    // Call backend API
                    fetch(url)
                        .then(response => {
                            if (!response.ok) {
                                throw new Error(`Server returned ${response.status}: ${response.statusText}`);
                            }
                            return response.json();
                        })
                        .then(data => {
                            console.log('Todos fetched successfully:', data);
                            
                            // Check if response has pagination metadata
                            if (data.metadata) {
                                this.totalItems = data.metadata.total || 0;
                                this.totalPages = Math.ceil(this.totalItems / this.itemsPerPage);
                                
                                // Update todos array
                                this.allTodos = data.items || [];
                            } else {
                                // Fallback if API doesn't provide metadata yet
                                this.allTodos = Array.isArray(data) ? data : [];
                                this.totalItems = this.allTodos.length;
                                this.totalPages = 1;
                            }
                            
                            // Calculate stats
                            if (this.activeFilter === 'all') {
                                // If we're only showing a page, these stats might be incomplete
                                this.total_todos = this.totalItems;
                                // These could be inaccurate if we're not seeing all todos
                                this.completed_todos = this.allTodos.filter(t => t.completed).length;
                                this.pending_todos = this.total_todos - this.completed_todos;
                            } else if (this.activeFilter === 'completed') {
                                this.total_todos = this.totalItems; // Total items count
                                this.completed_todos = this.totalItems;
                                this.pending_todos = 0;
                            } else { // active
                                this.total_todos = this.totalItems; 
                                this.completed_todos = 0;
                                this.pending_todos = this.totalItems;
                            }
                            
                            // Apply filter to create the displayed todos array
                            this.applyFilter();
                            
                            // Show success toast
                            this.showToast('Success', 'Todos loaded successfully!', 'success');
                        })
                        .catch(error => {
                            console.error('Error fetching todos:', error);
                            this.errorMessage = 'Failed to load todos from the server.';
                            this.errorDetails = error.toString();
                        })
                        .finally(() => {
                            // Hide loading state
                            Alpine.store('uiStore').isLoading = false;
                        });
                },
                
                // Navigate to a specific page
                goToPage(page) {
                    if (page < 1 || page > this.totalPages || page === this.currentPage) {
                        return; // Invalid page or same page
                    }
                    
                    this.currentPage = page;
                    
                    // Update URL to reflect the page change
                    const url = new URL(window.location);
                    url.searchParams.set('page', page);
                    window.history.pushState({}, '', url);
                    
                    // Fetch todos for the new page
                    this.fetchInitialTodos();
                },
                
                // Get array of page numbers for pagination
                getPageNumbers() {
                    const pages = [];
                    const maxVisiblePages = 5;
                    
                    if (this.totalPages <= maxVisiblePages) {
                        // Show all pages if there are few
                        for (let i = 1; i <= this.totalPages; i++) {
                            pages.push(i);
                        }
                    } else {
                        // Show a subset of pages with current page centered
                        let start = Math.max(1, this.currentPage - Math.floor(maxVisiblePages / 2));
                        let end = Math.min(this.totalPages, start + maxVisiblePages - 1);
                        
                        // Adjust start if we're near the end
                        if (end === this.totalPages) {
                            start = Math.max(1, end - maxVisiblePages + 1);
                        }
                        
                        // Add first page
                        if (start > 1) {
                            pages.push(1);
                            if (start > 2) pages.push('...'); // Add ellipsis if needed
                        }
                        
                        // Add pages around current page
                        for (let i = start; i <= end; i++) {
                            pages.push(i);
                        }
                        
                        // Add last page
                        if (end < this.totalPages) {
                            if (end < this.totalPages - 1) pages.push('...'); // Add ellipsis if needed
                            pages.push(this.totalPages);
                        }
                    }
                    
                    return pages;
                },
                
                // Show toast notification
                showToast(title, message, type = 'info') {
                    console.log(`showToast: ${title} - ${message} (${type})`);
                    
                    // Create toast element
                    const toastContainer = document.querySelector('.toast-container');
                    if (!toastContainer) return;
                    
                    const toastEl = document.createElement('div');
                    toastEl.className = `toast align-items-center text-bg-${type} border-0`;
                    toastEl.setAttribute('role', 'alert');
                    toastEl.setAttribute('aria-live', 'assertive');
                    toastEl.setAttribute('aria-atomic', 'true');
                    
                    toastEl.innerHTML = `
                        <div class="d-flex">
                            <div class="toast-body">
                                <strong>${title}</strong>: ${message}
                            </div>
                            <button type="button" class="btn-close btn-close-white me-2 m-auto" data-bs-dismiss="toast" aria-label="Close"></button>
                        </div>
                    `;
                    
                    toastContainer.appendChild(toastEl);
                    
                    // Initialize and show the toast
                    const toast = new bootstrap.Toast(toastEl, { delay: 5000 });
                    toast.show();
                    
                    // Auto-remove after it's hidden
                    toastEl.addEventListener('hidden.bs.toast', () => {
                        toastEl.remove();
                    });
                },
                
                filterTodos(filter) { 
                    console.log('filterTodos called with:', filter);
                    if (filter === 'all') {
                        window.location.href = '/';
                    } else {
                        window.location.href = '/?completed=' + (filter === 'completed' ? 'true' : (filter === 'active' ? 'false' : '')); 
                    }
                },
                
                addTodo() { 
                    console.log('addTodo called');
                    
                    // Basic validation
                    if (!this.newTodo || this.newTodo.trim() === '') {
                        return; // Form validation should catch this already, but just in case
                    }
                    
                    // Show loading state
                    Alpine.store('uiStore').isLoading = true;
                    
                    // Create new todo object
                    const newTodoItem = {
                        todo: this.newTodo.trim(),
                        completed: false,
                        priority: 3, // Medium priority by default
                        category: 'general' // Default category
                    };
                    
                    // Send POST request to create new todo
                    fetch('/todos', {
                        method: 'POST',
                        headers: {
                            'Content-Type': 'application/json'
                        },
                        body: JSON.stringify(newTodoItem)
                    })
                    .then(response => {
                        if (!response.ok) {
                            throw new Error(`Server returned ${response.status}: ${response.statusText}`);
                        }
                        return response.json();
                    })
                    .then(data => {
                        console.log('Todo created successfully:', data);
                        
                        // Clear the input field
                        this.newTodo = '';
                        
                        // Add to allTodos array
                        this.allTodos.push(data);
                        
                        // Add to filtered todos array if it matches the current filter
                        if (this.activeFilter === 'all' || 
                           (this.activeFilter === 'active' && !data.completed) || 
                           (this.activeFilter === 'completed' && data.completed)) {
                            this.todos.push(data);
                        }
                        
                        // Update stats
                        this.total_todos = this.allTodos.length;
                        this.completed_todos = this.allTodos.filter(t => t.completed).length;
                        this.pending_todos = this.total_todos - this.completed_todos;
                        
                        // Show success toast
                        this.showToast('Success', 'Task added successfully!', 'success');
                        
                        // Hide loading state
                        Alpine.store('uiStore').isLoading = false;
                    })
                    .catch(error => {
                        console.error('Error creating todo:', error);
                        this.errorMessage = 'Failed to add the new task.';
                        this.errorDetails = error.toString();
                        
                        // Hide loading state on error
                        Alpine.store('uiStore').isLoading = false;
                    });
                },
                
                updateTodo(id, completed) { 
                    console.log('updateTodo called:', id, completed);
                    
                    // Find the todo in our arrays
                    const todoInAll = this.allTodos.find(t => t.id === id);
                    const todoInFiltered = this.todos.find(t => t.id === id);
                    
                    if (!todoInAll) {
                        console.error(`Todo with ID ${id} not found in allTodos array`);
                        return;
                    }
                    
                    // Show loading state
                    Alpine.store('uiStore').isLoading = true;
                    
                    // Send PUT request to update the todo
                    fetch(`/todos/${id}`, {
                        method: 'PUT',
                        headers: {
                            'Content-Type': 'application/json'
                        },
                        body: JSON.stringify({ completed: completed })
                    })
                    .then(response => {
                        if (!response.ok) {
                            throw new Error(`Server returned ${response.status}: ${response.statusText}`);
                        }
                        return response.json();
                    })
                    .then(data => {
                        console.log('Todo updated successfully:', data);
                        
                        // Update in allTodos array
                        todoInAll.completed = completed;
                        
                        // Update in filtered todos array if present
                        if (todoInFiltered) {
                            todoInFiltered.completed = completed;
                        }
                        
                        // Item might need to be removed from filtered view if it no longer matches filter
                        if (this.activeFilter !== 'all') {
                            // If we're viewing only active and the todo was marked as completed, or vice versa
                            const shouldBeVisible = this.activeFilter === 'completed' ? completed : !completed;
                            
                            if (!shouldBeVisible && todoInFiltered) {
                                // Remove it from filtered view (will disappear from UI)
                                this.todos = this.todos.filter(t => t.id !== id);
                            }
                        }
                        
                        // Update stats
                        this.completed_todos = this.allTodos.filter(t => t.completed).length;
                        this.pending_todos = this.allTodos.length - this.completed_todos;
                        
                        // Show success toast
                        this.showToast('Success', `Task marked as ${completed ? 'completed' : 'active'}`, 'success');
                        
                        // Hide loading state
                        Alpine.store('uiStore').isLoading = false;
                    })
                    .catch(error => {
                        console.error('Error updating todo:', error);
                        this.errorMessage = `Failed to update task status.`;
                        this.errorDetails = error.toString();
                        
                        // Revert the checkbox state in the UI
                        if (todoInAll) todoInAll.completed = !completed;
                        if (todoInFiltered) todoInFiltered.completed = !completed;
                        
                        // Hide loading state on error
                        Alpine.store('uiStore').isLoading = false;
                    });
                },
                
                clearCompleted() { 
                    console.log('clearCompleted called');
                    
                    // Use allTodos to find completed ones regardless of current filter
                    const completedTodos = this.allTodos.filter(t => t.completed);
                    console.log('Found completed todos:', completedTodos.length, completedTodos);
                    
                    if (completedTodos.length === 0) {
                        this.showToast('Info', 'No completed tasks to clear', 'info');
                        return;
                    }
                    
                    // Show loading state
                    Alpine.store('uiStore').isLoading = true;
                    
                    // Keep track of successful and failed deletions
                    let successCount = 0;
                    let failureCount = 0;
                    let pendingCount = completedTodos.length;
                    
                    // Create an array of completed todo IDs to remove from arrays after API calls
                    const completedIDs = completedTodos.map(todo => todo.id);
                    
                    // For each completed todo, send a delete request
                    completedTodos.forEach(todo => {
                        fetch(`/todos/${todo.id}`, {
                            method: 'DELETE',
                            headers: {
                                'Content-Type': 'application/json'
                            }
                        })
                        .then(response => {
                            if (!response.ok) {
                                throw new Error(`Failed to delete todo ${todo.id}: ${response.status}`);
                            }
                            
                            // Deletion successful, remove from local array
                            successCount++;
                            console.log(`Successfully deleted todo ${todo.id}`);
                        })
                        .catch(error => {
                            failureCount++;
                            console.error(`Error deleting todo ${todo.id}:`, error);
                        })
                        .finally(() => {
                            pendingCount--;
                            
                            // If all deletion requests have completed, update UI and show result
                            if (pendingCount === 0) {
                                // If there were successful deletions, update arrays
                                if (successCount > 0) {
                                    // Remove successfully deleted todos from allTodos
                                    this.allTodos = this.allTodos.filter(todo => 
                                        !completedIDs.includes(todo.id) || 
                                        !todo.completed // Keep incomplete todos even if in completedIDs (shouldn't happen but as safety)
                                    );
                                    
                                    // Remove from filtered array as well
                                    this.todos = this.todos.filter(todo => 
                                        !completedIDs.includes(todo.id) || 
                                        !todo.completed
                                    );
                                    
                                    // Update stats
                                    this.total_todos = this.allTodos.length;
                                    this.completed_todos = this.allTodos.filter(t => t.completed).length;
                                    this.pending_todos = this.total_todos - this.completed_todos;
                                }
                                
                                // Show result toast
                                if (failureCount === 0) {
                                    this.showToast('Success', `Cleared ${successCount} completed tasks`, 'success');
                                } else if (successCount === 0) {
                                    this.showToast('Error', `Failed to clear completed tasks`, 'danger');
                                } else {
                                    this.showToast('Warning', `Cleared ${successCount} tasks, but failed to clear ${failureCount} tasks`, 'warning');
                                }
                                
                                // Hide loading state
                                Alpine.store('uiStore').isLoading = false;
                            }
                        });
                    });
                },
                
                retryLastOperation() { 
                    console.log('retryLastOperation called'); 
                    this.fetchInitialTodos();
                },
                
                clearError() { 
                    this.errorMessage = null; 
                    this.errorDetails = null; 
                    console.log('clearError called'); 
                },
                
                formatDate(d) { 
                    return d ? new Date(d).toLocaleDateString() : '-'; 
                }
            };
        });
        console.log('app.js: todoApp component registered');
    } catch (e) {
        console.error('Error registering todoApp component:', e);
    }
}); 